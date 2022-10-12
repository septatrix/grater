use std::io::Read;

use serde::{Deserialize, Serialize};
use tabula::{ExtractionMethod, OutputFormat, TabulaEnv, TabulaVM};

#[derive(Serialize, Deserialize, Debug)]
struct CellOutput {
    top: f32,
    left: f32,
    width: f32,
    height: f32,
    text: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct TableOutput {
    extraction_method: String, // TODO make enum
    page_number: u32,
    top: f32,
    left: f32,
    width: f32,
    height: f32,
    right: f32,
    bottom: f32,
    // List of rows with cells
    data: Vec<Vec<CellOutput>>, // TODO rename but keep name for serde
}

const UNGRADED_MODULES: [&str; 1] = ["Software-Projektpraktikum"];
const EXCLUDED_MODULES: [&str; 4] = [
    "Mathematische Logik I",

    "Analysis für Informatik",
    // "Diskrete Strukturen",

    "Einführung in das wissenschaftliche Arbeiten (Proseminar Informatik)",
    // "Programmierung",

    "Grundgebiete der Elektrotechnik 2 - Modellierung und Analyse elektrischer Komponenten und Schaltungen",
];

fn main() {
    let vm = TabulaVM::new("/tmp/septatrix-playground-2022-09-21T21-25-57-HQe/tabula-java/target/tabula-1.0.6-SNAPSHOT-jar-with-dependencies.jar", true).unwrap();
    let env = vm.attach().unwrap();

    parse_kontoauszug(env);
}

fn _parse_notespiegel(env: TabulaEnv) -> () {
    let tabula = env
        .configure_tabula(
            None,
            Some(&[1, 2, 3]),
            OutputFormat::Json,
            true,
            ExtractionMethod::Decide,
            false,
            None,
        )
        .unwrap();
    let mut file = tabula
        .parse_document(
            &std::path::Path::new(
                "./samples/Notenspiegel_(Alles)_21092022_2035_4635318_20220921203552.pdf",
            ),
            "foobar",
        )
        .unwrap();
    let mut fin = String::new();
    file.read_to_string(&mut fin).unwrap();

    let output: Vec<TableOutput> = serde_json::from_str(fin.as_str()).unwrap();

    let mut credits = 0.0;
    let mut grade_sum = 0.0;

    for table in output {
        for row in table.data {
            // TODO use #![feature(let_else)] one stabilized
            if let [ref module, ref grade, ref annotation, ref _recognized, ref credit_points, ref _date, ref _semester] =
                row.as_slice()
            {
                if annotation.text != "BE" {
                    continue;
                }
                println!(
                    "{}\t{}\t{}\t{}",
                    grade.text,
                    credit_points.text,
                    annotation.text,
                    module.text.replace('\r', " ")
                );
                let credit_weight: f32 = credit_points.text.replace(',', ".").parse().unwrap();
                credits += credit_weight;
                if let Ok(grade) = grade.text.replace(',', ".").parse::<f32>() {
                    grade_sum += grade * credit_weight;
                }
            }
        }
    }

    println!("{} - {}", credits, grade_sum / credits);
}

fn parse_kontoauszug(env: TabulaEnv) -> () {
    let tabula = env
        .configure_tabula(
            None,
            Some(&[1, 2, 3, 4]),
            OutputFormat::Json,
            true,
            ExtractionMethod::Spreadsheet,
            false,
            None,
        )
        .unwrap();
    let mut file = tabula
        .parse_document(
            // &std::path::Path::new("./samples/Kontoauszug_21092022_2035_4635315_20220921203533.pdf"),
            &std::path::Path::new("./samples/Kontoauszug_23092022_1851_4644333_20220923185119.pdf"),
            "foobar",
        )
        .unwrap();
    let mut fin = String::new();
    file.read_to_string(&mut fin).unwrap();

    let output: Vec<TableOutput> = serde_json::from_str(fin.as_str()).unwrap();

    let mut total_credits = 0.0;
    let mut graded_credits = 0.0;
    let mut grade_sum = 0.0;

    let mut sections = Vec::<(String, f32)>::new();

    'outer: for row in output.into_iter().flat_map(|table| table.data) {
        // TODO use #![feature(let_else)] once stabilized
        // Modul-ID Typ Module/Fächer Note Vm Ang CP Datum Sem
        if let [ref _module_id, ref kind, ref module, ref grade, ref _annotation, ref _recognized, ref credit_points, ref _date, ref _semester] =
            row.as_slice()
        {
            let module_name = module.text.replace('\r', " ");
            let credit_weight: f32 = credit_points.text.replace(',', ".").parse().unwrap_or(0.0);

            // Module groups
            if kind.text == "RK" {
                let section_name = module_name;
                if section_name == "Mastervorzugsfächer" {
                    break 'outer;
                }
                let credit_weight: f32 =
                    credit_points.text.replace(',', ".").parse().unwrap_or(6f32);
                println!("{}* **{}**", " ".repeat(2 * sections.len()), section_name);
                sections.push((section_name, credit_weight));
                continue;
            }

            // Skip miscellaneous rows like failed/unfinished modules
            if kind.text != "MK" || grade.text == "" {
                continue;
            }

            total_credits += credit_weight;

            if [&EXCLUDED_MODULES[..], &UNGRADED_MODULES[..]]
                .concat()
                .contains(&module_name.as_str())
                || grade.text == "B"
            {
                eprintln!(
                    "Skipped {:40}, ungraded? {:5}, excluded? {:5}, ({})",
                    module_name,
                    grade.text == "B",
                    EXCLUDED_MODULES.contains(&module_name.as_str()),
                    credit_weight,
                );
                continue;
            }

            graded_credits += credit_weight;
            grade_sum += credit_weight * grade.text.replace(',', ".").parse::<f32>().unwrap();

            println!(
                "{}* {} @ {} ({})",
                " ".repeat(2 * sections.len()),
                module_name,
                grade.text,
                credit_points.text,
            );
            sections = sections
                .into_iter()
                .map_while(|(name, remainaing_credits)| {
                    let remainaing_credits = remainaing_credits - credit_weight;

                    if remainaing_credits == 0.0 {
                        return None;
                    }
                    Some((name, remainaing_credits))
                })
                .collect();
        }
    }

    total_credits += 15.0;
    graded_credits += 15.0;
    grade_sum += 12.0 * 2.0;
    grade_sum += 3.0 * 2.3;

    println!("\n{} - {}", total_credits, grade_sum / graded_credits);
    eprintln!("\n{} - {}", total_credits, grade_sum / graded_credits);
}
