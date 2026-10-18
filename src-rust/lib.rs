use std::panic;
use std::{cmp::Ordering, collections::HashMap, fmt::Write};

use itertools::Itertools;
use phf::{phf_map, Map};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
enum Grade {
    Passed,
    Numeric(f32),
}

#[allow(dead_code)]
#[wasm_bindgen]
#[derive(Debug, Serialize, Deserialize)]
pub struct Module {
    category: String,
    label: String,
    grade: Grade,
    credits: f32,
    weight_modifier: f32,
}

/// The degree programme whose examination regulations apply.
///
/// Rules are taken from the Informatik regulations
/// (Bachelor: Prüfungsordnungsversion 2022, Master: version 2023,
/// both consolidated on 2025-09-18)
/// and the ÜPO (§ 10 Abs. 8, 11 and 12).
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Degree {
    Bachelor,
    Master,
}

impl Degree {
    /// Credits of the thesis module including the colloquium.
    fn thesis_credits(self) -> f32 {
        match self {
            Degree::Bachelor => 15.0,
            Degree::Master => 30.0,
        }
    }

    /// Upper bound for the credits of struck module grades
    /// (§ 10 Abs. 5 sPO Bachelor, § 8 Abs. 5 sPO Master).
    fn strikeable_credit_total(self) -> f32 {
        match self {
            Degree::Bachelor => 30.0,
            Degree::Master => 15.0,
        }
    }

    /// Whether more than one grade may be struck.
    /// In the Bachelor, one grade per area may be struck,
    /// in the Master only a single grade overall.
    fn strikes_per_category(self) -> bool {
        self == Degree::Bachelor
    }

    fn module_weight(self, label: &str) -> f32 {
        match self {
            Degree::Bachelor => BACHELOR_MODULE_WEIGHT.get(label),
            Degree::Master => MASTER_MODULE_WEIGHT.get(label),
        }
        .copied()
        .unwrap_or(1.0)
    }
}

/// Modules whose grade can never be struck.
/// The thesis modules are covered by their category.
const UNSTRIKEABLE_MODULES: [&str; 1] = ["Schwerpunktkolloquium"];

const BACHELOR_MODULE_WEIGHT: Map<&'static str, f32> = phf_map!(
    "Bachelorarbeit" => 1.5,
    "Kolloquium" => 1.5,
    // TODO Most of these are "B" anyhow, maybe improve this?
    // TODO also include language courses
    "Software-Projektpraktikum" => 0.0,
    "Systemprogrammierung" => 0.0,
    "Nicht-technisches Wahlfach Mentoring" => 0.0,
);
const MASTER_MODULE_WEIGHT: Map<&'static str, f32> = phf_map!(
    // § 8 Abs. 4 sPO Master: the oral colloquium counts four times its credits
    "Schwerpunktkolloquium" => 4.0,
);

#[wasm_bindgen(start)]
pub fn set_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    panic::set_hook(Box::new(console_error_panic_hook::hook));
}

#[wasm_bindgen]
pub fn extract_modules(input: JsValue, degree: Degree) -> Result<JsValue, JsValue> {
    let output = serde_wasm_bindgen::from_value(input)?;
    let modules = parse_tabula_output(output, degree);
    Ok(serde_wasm_bindgen::to_value(&modules)?)
}

fn parse_tabula_output(output: Vec<Vec<Vec<String>>>, degree: Degree) -> Vec<Module> {
    let mut curr_section = None;
    let mut modules = Vec::new();

    for row in output.into_iter().flatten() {
        // Modul-ID Typ Module/Fächer Note Vm Ang CP Datum Sem
        let (kind_text, module, grade, credit_points) = match row.as_slice() {
            [_module_id, kind, module, grade, _annotation, _recognized, credit_points, _date, _semester] => {
                (kind.clone(), module, grade, credit_points)
            }
            [_module_id, module, grade, _annotation, _recognized, credit_points, _date, _semester] =>
            {
                if module == "Abschlussarbeit" {
                    curr_section = Some((module.clone(), degree.thesis_credits()));
                    continue;
                }
                // We assume that this is the Abschlussarbeit section.
                // Otherwise the layout must have been misparsed.
                assert!(
                    curr_section.as_ref().expect("Current section must exist").0
                        == "Abschlussarbeit",
                    "Current section must be 'Abschlussarbeit' with expected layout"
                );
                ("MK".to_string(), module, grade, credit_points)
            }
            _ => continue,
        };

        let module_name = module.replace('\n', " ");
        let module_credits = credit_points.replace(',', ".").parse().unwrap_or(0.0);

        // Module groups
        if kind_text == "RK"
            && (curr_section.is_none()
                || curr_section
                    .as_ref()
                    .is_some_and(|s| s.1 == 0.0 && module_credits > 0.0))
        {
            let section_name = module_name;
            curr_section = Some((section_name, module_credits));
            continue;
        }

        // Skip miscellaneous rows like failed/unfinished modules
        if kind_text != "MK" || grade.is_empty() {
            continue;
        }

        let section = curr_section.as_ref().expect("Section should exist");
        // This looks like a module, but skip it if it is a Vorzugsfach
        if section.0 == "Mastervorzugsfächer" {
            continue;
        }

        let module = Module {
            category: section.0.clone(),
            label: module_name.clone(),
            grade: grade
                .replace(',', ".")
                .parse::<f32>()
                // TODO assert that grade is "B" in case of parsing error
                .map_or(Grade::Passed, Grade::Numeric),
            credits: module_credits,
            weight_modifier: degree.module_weight(&module_name),
        };
        //println!("{:?}", module);
        modules.push(module);

        curr_section = Some((section.0.clone(), section.1 - module_credits));
    }

    modules
}

#[wasm_bindgen]
pub fn calculate_best_grade(input: JsValue, degree: Degree) -> Result<JsValue, JsValue> {
    let modules: Vec<Module> = serde_wasm_bindgen::from_value(input)?;
    let result = calculate_best_strike_combination(&modules, degree);
    Ok(JsValue::from_str(&result))
}

/// Truncates (not rounds!) to one decimal place as required by § 10 Abs. 11 ÜPO.
fn truncate_grade(grade: f32) -> f32 {
    // The epsilon compensates for float error, e.g. 1.3 being computed as 1.2999999
    ((grade * 10.0 + 1e-3).floor()) / 10.0
}

fn calculate_best_strike_combination(modules: &[Module], degree: Degree) -> String {
    let (best_grade, best_combinations) = find_best_strike_combinations(modules, degree);

    let mut summary = String::new();
    writeln!(summary, "Best possible grade: {:.1}", best_grade).unwrap();
    writeln!(
        summary,
        "The following strike combinations lead to this grade:"
    )
    .unwrap();

    for combi in best_combinations {
        write!(summary, "{:#?}", combi).unwrap();
    }

    summary
}

fn find_best_strike_combinations(modules: &[Module], degree: Degree) -> (f32, Vec<Vec<&String>>) {
    // TODO maybe replace with itertools.group_by if sorting is guaranteed
    let mut strike_candidates_by_group = HashMap::<_, Vec<_>>::new();
    for module in modules.iter() {
        // Thesis modules and the Schwerpunktkolloquium cannot be struck
        // Striking ungraded modules and modules with weight zero is useless
        // (modules have weight zero iff they are never included in the grade,
        // e.g. Software-Projektpraktikum )
        if module.category == "Abschlussarbeit"
            || UNSTRIKEABLE_MODULES.iter().any(|m| module.label.contains(m))
            || module.grade == Grade::Passed
            || module.weight_modifier == 0.0
        {
            continue;
        }
        // Bachelor: at most one module per area can be struck.
        // Master: at most one module in total can be struck,
        // which is modelled as a single group.
        let group = if degree.strikes_per_category() {
            module.category.as_str()
        } else {
            ""
        };
        strike_candidates_by_group
            .entry(group)
            .or_default()
            .push(module);
    }

    let max_group_count = if degree.strikes_per_category() {
        strike_candidates_by_group.len()
    } else {
        1.min(strike_candidates_by_group.len())
    };

    let strike_combinations = (0..=max_group_count).flat_map(|count| {
        strike_candidates_by_group
            .values()
            .combinations(count)
            .flat_map(|cat_comb| cat_comb.into_iter().multi_cartesian_product())
            .filter(|combi| {
                combi.iter().map(|module| module.credits).sum::<f32>()
                    <= degree.strikeable_credit_total()
            })
            .map(|combi| {
                combi
                    .into_iter()
                    .map(|module| &module.label)
                    .collect::<Vec<_>>()
            })
    });

    let grade_results = strike_combinations
        .map(|combi| {
            let grade = calculate_grade(modules.iter().filter(|m| !combi.contains(&&m.label)));
            (combi, truncate_grade(grade))
        })
        .collect::<Vec<_>>();

    let best_grade = grade_results
        .iter()
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal))
        .expect("Best grade should exist")
        .1;

    let best_combinations = grade_results
        .into_iter()
        .filter(|(_, grade)| grade == &best_grade)
        .map(|(combi, _)| combi)
        .collect();

    (best_grade, best_combinations)
}

fn calculate_grade<'a>(modules: impl IntoIterator<Item = &'a Module>) -> f32 {
    let mut graded_credits = 0.0;
    let mut grade_sum = 0.0;

    for module in modules {
        if let Grade::Numeric(g) = module.grade {
            graded_credits += module.credits * module.weight_modifier;
            grade_sum += g * module.credits * module.weight_modifier;
        }
    }

    grade_sum / graded_credits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(category: &str, label: &str, grade: f32, credits: f32, weight: f32) -> Module {
        Module {
            category: category.into(),
            label: label.into(),
            grade: Grade::Numeric(grade),
            credits,
            weight_modifier: weight,
        }
    }

    #[test]
    fn grades_are_truncated() {
        assert_eq!(truncate_grade(1.39), 1.3);
        assert_eq!(truncate_grade(1.3), 1.3);
        assert_eq!(truncate_grade(2.0), 2.0);
    }

    #[test]
    fn bachelor_strikes_one_per_area_up_to_30_cp() {
        let modules = [
            module("A", "a1", 4.0, 8.0, 1.0),
            module("A", "a2", 4.0, 8.0, 1.0),
            module("B", "b1", 4.0, 8.0, 1.0),
            module("C", "c1", 1.0, 8.0, 1.0),
            module("Abschlussarbeit", "Bachelorarbeit", 1.0, 15.0, 1.5),
        ];
        let (_, combis) = find_best_strike_combinations(&modules, Degree::Bachelor);
        assert!(!combis.is_empty());
        for combi in combis {
            // One of a1/a2 and b1 are struck, never both a1 and a2
            assert!(combi.contains(&&"b1".to_string()));
            assert_eq!(combi.len(), 2);
        }
    }

    #[test]
    fn master_strikes_a_single_module_up_to_15_cp() {
        let modules = [
            module("A", "a1", 4.0, 8.0, 1.0),
            module("B", "b1", 4.0, 8.0, 1.0),
            module("C", "big", 5.0, 18.0, 1.0),
            module("C", "Schwerpunktkolloquium", 5.0, 3.0, 4.0),
            module("Abschlussarbeit", "Masterarbeit", 1.0, 30.0, 1.0),
        ];
        let (_, combis) = find_best_strike_combinations(&modules, Degree::Master);
        // "big" exceeds 15 CP and the Schwerpunktkolloquium cannot be struck
        assert!(!combis.is_empty());
        for combi in combis {
            assert_eq!(combi.len(), 1);
            assert!(combi[0] == "a1" || combi[0] == "b1");
        }
    }
}
