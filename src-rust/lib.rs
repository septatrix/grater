#![feature(is_some_and)]
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
    //#[cfg(feature = "console_error_panic_hook")]
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

    for row in output.into_iter().flatten() {
        // TODO use #![feature(let_else)] once stabilized
        // Modul-ID Typ Module/Fächer Note Vm Ang CP Datum Sem
        let (kind_text, module, grade, credit_points) = if let [ref _module_id, ref kind, ref module, ref grade, ref _annotation, ref _recognized, ref credit_points, ref _date, ref _semester] =
            row.as_slice()
        {
            (kind.clone(), module, grade, credit_points)
        } else if let [ref _module_id, ref module, ref grade, ref _annotation, ref _recognized, ref credit_points, ref _date, ref _semester] =
            row.as_slice()
        {
            if module == "Abschlussarbeit" {
                curr_section = Some((module.clone(), 15.0));
                continue;
            }
            // We assume that this is the Abschlussarbeit section.
            // Otherwise the layout must have been misparsed.
            assert!(
                curr_section.as_ref().expect("Current section must exist").0 == "Abschlussarbeit",
                "Current section must be 'Abschlussarbeit' with expected layout"
            );
            ("MK".to_string(), module, grade, credit_points)
        } else {
            continue;
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

        let section = curr_section.as_ref().unwrap();
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
            weight_modifier: *MODULE_WEIGHT.get(&module_name).unwrap_or(&1.0),
        };
        //println!("{:?}", module);
        modules.push(module);

        curr_section = Some((section.0.clone(), section.1 - module_credits));
    }

    modules
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
            // TODO is this clone useless?
            .entry(module.category.clone())
            .or_default()
            .push(module);
    }

    let strike_combinations = (0..=strike_candidates_by_category.len()).flat_map(|count| {
        strike_candidates_by_category
            .values()
            .combinations(count)
            .flat_map(|cat_comb| cat_comb.into_iter().multi_cartesian_product())
            .filter(|combi| combi.iter().map(|module| module.credits).sum::<f32>() <= 30.0)
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
        .unwrap()
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
