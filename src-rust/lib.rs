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
    /// Date of the exam as `YYYYMMDD` to allow chronological sorting.
    /// Empty if the date could not be determined.
    #[serde(default)]
    sort_date: String,
}

const STRIKEABLE_CREDIT_TOTAL: f32 = 30.0;
const MODULE_WEIGHT: Map<&'static str, f32> = phf_map!(
    "Bachelorarbeit" => 1.5,
    "Kolloquium" => 1.5,
    // TODO Most of these are "B" anyhow, maybe improve this?
    // TODO also include language courses
    "Software-Projektpraktikum" => 0.0,
    "Systemprogrammierung" => 0.0,
    "Nicht-technisches Wahlfach Mentoring" => 0.0,
);

#[wasm_bindgen(start)]
pub fn set_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    panic::set_hook(Box::new(console_error_panic_hook::hook));
}

#[wasm_bindgen]
pub fn extract_modules(input: JsValue) -> Result<JsValue, JsValue> {
    let output = serde_wasm_bindgen::from_value(input)?;
    let modules = parse_tabula_output(output);
    calculate_best_strike_combination(&modules);
    Ok(serde_wasm_bindgen::to_value(&modules)?)
}

fn parse_tabula_output(output: Vec<Vec<Vec<String>>>) -> Vec<Module> {
    let mut curr_section = None;
    let mut modules = Vec::new();
    // Total credits required per section as announced by the section header
    let mut section_credits = HashMap::<String, f32>::new();

    for row in output.into_iter().flatten() {
        // Modul-ID Typ Module/Fächer Note Vm Ang CP Datum Sem
        let (kind_text, module, grade, credit_points, date) = match row.as_slice() {
            [_module_id, kind, module, grade, _annotation, _recognized, credit_points, date, _semester] => {
                (kind.clone(), module, grade, credit_points, date)
            }
            [_module_id, module, grade, _annotation, _recognized, credit_points, date, _semester] =>
            {
                if module == "Abschlussarbeit" {
                    curr_section = Some((module.clone(), 15.0));
                    continue;
                }
                // We assume that this is the Abschlussarbeit section.
                // Otherwise the layout must have been misparsed.
                assert!(
                    curr_section.as_ref().expect("Current section must exist").0
                        == "Abschlussarbeit",
                    "Current section must be 'Abschlussarbeit' with expected layout"
                );
                ("MK".to_string(), module, grade, credit_points, date)
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
            section_credits.insert(section_name.clone(), module_credits);
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
            weight_modifier: module_weight(&module_name),
            sort_date: sortable_date(date),
        };
        //println!("{:?}", module);
        modules.push(module);

        curr_section = Some((section.0.clone(), section.1 - module_credits));
    }

    ignore_excess_elective_modules(&mut modules, &section_credits);

    modules
}

/// Weight of a module in the final grade.
fn module_weight(name: &str) -> f32 {
    // § 10 Abs. 4 PO: All Nicht-Technischen Wahlfächer are ignored
    if name.to_lowercase().starts_with("nicht-technisch") {
        return 0.0;
    }
    *MODULE_WEIGHT.get(name).unwrap_or(&1.0)
}

/// Converts `DD.MM.YYYY` into `YYYYMMDD`.
fn sortable_date(date: &str) -> String {
    match date.trim().split('.').collect::<Vec<_>>().as_slice() {
        [d, m, y] => format!("{:0>4}{:0>2}{:0>2}", y, m, d),
        _ => String::new(),
    }
}

/// § 10 Abs. 4 PO: If more Wahlpflicht modules were taken than required,
/// the modules enter the final grade in the order in which the exams were taken.
/// Modules beyond the required credits get weight zero.
fn ignore_excess_elective_modules(modules: &mut [Module], section_credits: &HashMap<String, f32>) {
    for (section, required) in section_credits {
        if !section.to_lowercase().contains("wahlpflicht") {
            continue;
        }
        let mut indices = modules
            .iter()
            .positions(|m| &m.category == section && m.weight_modifier > 0.0)
            .collect::<Vec<_>>();
        if indices.iter().map(|&i| modules[i].credits).sum::<f32>() <= *required {
            continue;
        }
        // Stable sort, modules with unknown date keep their position
        indices.sort_by(|&a, &b| modules[a].sort_date.cmp(&modules[b].sort_date));

        let mut taken = 0.0;
        for i in indices {
            if taken >= *required {
                modules[i].weight_modifier = 0.0;
            }
            taken += modules[i].credits;
        }
    }
}

#[wasm_bindgen]
pub fn calculate_best_grade(input: JsValue) -> Result<JsValue, JsValue> {
    let modules: Vec<Module> = serde_wasm_bindgen::from_value(input)?;
    let result = calculate_best_strike_combination(&modules);
    Ok(JsValue::from_str(&result))
}

fn calculate_best_strike_combination(modules: &[Module]) -> String {
    // TODO maybe replace with itertools.group_by if sorting is guaranteed
    let mut strike_candidates_by_category = HashMap::<_, Vec<_>>::new();
    for module in modules.iter() {
        // Abschlussarbeiten cannot be striked
        // Striking ungraded modules and modules with weight zero is useless
        // (modules have weight zero iff they are never included in the grade,
        // e.g. Software-Projektpraktikum )
        if module.category == "Abschlussarbeit"
            || module.grade == Grade::Passed
            || module.weight_modifier == 0.0
        {
            continue;
        }
        strike_candidates_by_category
            .entry(&module.category)
            .or_default()
            .push(module);
    }

    let strike_combinations = (0..=strike_candidates_by_category.len()).flat_map(|count| {
        strike_candidates_by_category
            .values()
            .combinations(count)
            .flat_map(|cat_comb| cat_comb.into_iter().multi_cartesian_product())
            .filter(|combi| {
                combi.iter().map(|module| module.credits).sum::<f32>() <= STRIKEABLE_CREDIT_TOTAL
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
            (combi, grade)
        })
        .collect::<Vec<_>>();

    let best_grade = grade_results
        .iter()
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal))
        .expect("Best grade should exist")
        .1;

    let mut summary = String::new();
    writeln!(summary, "Best possible grade: {}", best_grade).unwrap();
    writeln!(
        summary,
        "The following strike combinations lead to this grade:"
    )
    .unwrap();

    for (combi, _) in grade_results
        .into_iter()
        .filter(|(_, grade)| grade == &best_grade)
    {
        write!(summary, "{:#?}", combi).unwrap();
    }

    summary
}

fn calculate_grade<'a>(modules: impl IntoIterator<Item = &'a Module>) -> f32 {
    // let mut total_credits = 0.0;
    let mut graded_credits = 0.0;
    let mut grade_sum = 0.0;

    for module in modules {
        // total_credits += module.credits;
        if let Grade::Numeric(g) = module.grade {
            graded_credits += module.credits * module.weight_modifier;
            grade_sum += g * module.credits * module.weight_modifier;
        }
    }

    // println!("{} - {}", total_credits, grade_sum / graded_credits);

    grade_sum / graded_credits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn elective(label: &str, credits: f32, date: &str) -> Module {
        Module {
            category: "Wahlpflichtbereich".to_string(),
            label: label.to_string(),
            grade: Grade::Numeric(2.0),
            credits,
            weight_modifier: 1.0,
            sort_date: sortable_date(date),
        }
    }

    #[test]
    fn nontechnical_electives_are_ignored() {
        assert_eq!(module_weight("Nicht-Technisches Wahlfach Rhetorik"), 0.0);
        assert_eq!(module_weight("Bachelorarbeit"), 1.5);
    }

    #[test]
    fn excess_electives_are_dropped_chronologically() {
        let mut modules = vec![
            elective("late", 6.0, "01.02.2024"),
            elective("early", 6.0, "01.02.2023"),
            elective("middle", 6.0, "15.07.2023"),
        ];
        let required = HashMap::from([("Wahlpflichtbereich".to_string(), 12.0)]);
        ignore_excess_elective_modules(&mut modules, &required);
        assert_eq!(modules[0].weight_modifier, 0.0);
        assert_eq!(modules[1].weight_modifier, 1.0);
        assert_eq!(modules[2].weight_modifier, 1.0);
    }
}
