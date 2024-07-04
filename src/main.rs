use copypasta::{ClipboardContext, ClipboardProvider};
use egui::{CentralPanel, Context, TopBottomPanel};
use egui_extras::{Column, TableBuilder};
use evalexpr::*;
use regex::Regex;
use reqwest;
use std::collections::HashMap;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Atmos+",
        options,
        Box::new(|_cc| Ok(Box::new(MyApp::default()))),
    )
}

#[derive(Default)]
struct MyApp {
    constants: HashMap<String, String>,
    user_variables: HashMap<String, String>,
    sorted_constants: Vec<(String, String)>,
    filtered_constants: Vec<(String, String)>,
    selected_tab: Tab,
    sort_order: SortOrder,
    calculator_input: String,
    calculator_output: String,
    search_query: String,
    new_variable_name: String,
    new_variable_value: String,
}

#[derive(PartialEq)]
enum Tab {
    ParseConstants,
    Calculator,
}

#[derive(PartialEq)]
enum SortOrder {
    Ascending,
    Descending,
}

impl Default for Tab {
    fn default() -> Self {
        Tab::ParseConstants
    }
}

impl Default for SortOrder {
    fn default() -> Self {
        SortOrder::Ascending
    }
}

impl MyApp {
    fn fetch_and_parse_constants(&mut self) {
        let url = "https://raw.githubusercontent.com/space-wizards/space-station-14/master/Content.Shared/Atmos/Atmospherics.cs";
        match reqwest::blocking::get(url) {
            Ok(response) => {
                if let Ok(text) = response.text() {
                    let re = Regex::new(r#"public const (\w+) (\w+) = ([^;]+);"#).unwrap();
                    self.constants.clear();
                    for cap in re.captures_iter(&text) {
                        self.constants.insert(cap[2].to_string(), cap[3].to_string());
                    }
                    self.sort_and_filter_constants();
                }
            }
            Err(err) => {
                eprintln!("Error fetching file: {:?}", err);
            }
        }
    }

    fn sort_and_filter_constants(&mut self) {
        let mut sorted: Vec<(String, String)> = self.constants.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        sorted.sort_by(|a, b| match self.sort_order {
            SortOrder::Ascending => a.0.cmp(&b.0),
            SortOrder::Descending => b.0.cmp(&a.0),
        });
        self.sorted_constants = sorted;
        self.filter_constants();
    }

    fn filter_constants(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_constants = self.sorted_constants.clone();
        } else {
            let query = self.search_query.to_lowercase();
            self.filtered_constants = self.sorted_constants.iter()
                .filter(|(k, v)| k.to_lowercase().contains(&query) || v.to_lowercase().contains(&query))
                .cloned()
                .collect();
        }
    }

    fn copy_to_clipboard(&self) {
        let mut ctx = ClipboardContext::new().unwrap();
        let mut clipboard_content = String::new();

        for (name, value) in &self.filtered_constants {
            clipboard_content.push_str(&format!("{}\t{}\n", name, value));
        }

        ctx.set_contents(clipboard_content).unwrap();
    }

    fn evaluate_expression(&self, expression: &str, context: &HashMapContext) -> Result<String, String> {
        match eval_with_context(expression, context) {
            Ok(value) => Ok(format!("{}", value)),
            Err(err) => Err(format!("Ошибка при вычислении: {}", err)),
        }
    }

    fn create_evaluation_context(&self) -> HashMapContext {
        let mut context = HashMapContext::new();

        for (name, value) in &self.constants {
            let cleaned_value = value.trim_end_matches('f');
            if let Ok(parsed_value) = cleaned_value.parse::<f64>() {
                context.set_value(name.clone(), Value::Float(parsed_value)).unwrap();
            } else {
                if let Ok(resolved_value) = self.resolve_expression(value, &mut context.clone()) {
                    if let Ok(parsed_value) = resolved_value.parse::<f64>() {
                        context.set_value(name.clone(), Value::Float(parsed_value)).unwrap();
                    }
                }
            }
        }

        for (name, value) in &self.user_variables {
            let cleaned_value = value.trim_end_matches('f');
            if let Ok(parsed_value) = cleaned_value.parse::<f64>() {
                context.set_value(name.clone(), Value::Float(parsed_value)).unwrap();
            } else {
                if let Ok(resolved_value) = self.resolve_expression(value, &mut context.clone()) {
                    if let Ok(parsed_value) = resolved_value.parse::<f64>() {
                        context.set_value(name.clone(), Value::Float(parsed_value)).unwrap();
                    }
                }
            }
        }

        context
    }

    fn resolve_expression(&self, expression: &str, context: &mut HashMapContext) -> Result<String, String> {
        let mut expr = expression.to_string();

        let number_with_f_regex = Regex::new(r"(\d+\.?\d*e[-+]?\d*|\d+\.?\d*)f").unwrap();
        expr = number_with_f_regex.replace_all(&expr, "$1").to_string();

        let mut unresolved = true;
        while unresolved {
            unresolved = false;
            for (name, value) in &self.constants {
                let re = Regex::new(&format!(r"\b{}\b", regex::escape(name))).unwrap();
                if re.is_match(&expr) {
                    unresolved = true;
                    let resolved_value = if let Ok(parsed_value) = value.trim_end_matches('f').parse::<f64>() {
                        parsed_value.to_string()
                    } else {
                        self.resolve_expression(value, context)?
                    };
                    expr = re.replace_all(&expr, resolved_value.as_str()).to_string();
                }
            }

            for (name, value) in &self.user_variables {
                let re = Regex::new(&format!(r"\b{}\b", regex::escape(name))).unwrap();
                if re.is_match(&expr) {
                    unresolved = true;
                    let resolved_value = if let Ok(parsed_value) = value.trim_end_matches('f').parse::<f64>() {
                        parsed_value.to_string()
                    } else {
                        self.resolve_expression(value, context)?
                    };
                    expr = re.replace_all(&expr, resolved_value.as_str()).to_string();
                }
            }
        }

        self.evaluate_expression(&expr, context)
    }

    fn delete_user_variable(&mut self, name: &str) {
        self.user_variables.remove(name);
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.selectable_label(self.selected_tab == Tab::ParseConstants, "Atmos Constants").clicked() {
                    self.selected_tab = Tab::ParseConstants;
                }
                if ui.selectable_label(self.selected_tab == Tab::Calculator, "Calculator").clicked() {
                    self.selected_tab = Tab::Calculator;
                }
            });
        });

        CentralPanel::default().show(ctx, |ui| {
            match self.selected_tab {
                Tab::ParseConstants => {
                    if self.constants.is_empty() {
                        if ui.button("Load Constants").clicked() {
                            self.fetch_and_parse_constants();
                        }
                    } else {
                        ui.horizontal(|ui| {
                            if ui.button("Sort Ascending").clicked() {
                                self.sort_order = SortOrder::Ascending;
                                self.sort_and_filter_constants();
                            }
                            if ui.button("Sort Descending").clicked() {
                                self.sort_order = SortOrder::Descending;
                                self.sort_and_filter_constants();
                            }
                            if ui.button("Copy to Clipboard").clicked() {
                                self.copy_to_clipboard();
                            }
                            ui.label("Search:");
                            if ui.text_edit_singleline(&mut self.search_query).changed() {
                                self.filter_constants();
                            }
                        });

                        egui::ScrollArea::both().show(ui, |ui| {
                            TableBuilder::new(ui)
                                .striped(true)
                                .resizable(true)
                                .column(Column::initial(150.0).resizable(true))
                                .column(Column::remainder().resizable(true))
                                .header(20.0, |mut header| {
                                    header.col(|ui| {
                                        ui.heading("Constant Name");
                                    });
                                    header.col(|ui| {
                                        ui.heading("Value");
                                    });
                                })
                                .body(|mut body| {
                                    for (name, value) in &self.filtered_constants {
                                        body.row(20.0, |mut row| {
                                            row.col(|ui| {
                                                ui.label(name);
                                            });
                                            row.col(|ui| {
                                                ui.label(value);
                                            });
                                        });
                                    }
                                });
                        });
                    }
                }
                Tab::Calculator => {
                    ui.heading("Calculator");
                    ui.horizontal(|ui| {
                        ui.label("Expression:");
                        ui.text_edit_singleline(&mut self.calculator_input);
                    });
                    if ui.button("Calculate").clicked() {
                        let context = self.create_evaluation_context();
                        self.calculator_output = match self.resolve_expression(&self.calculator_input, &mut context.clone()) {
                            Ok(result) => result,
                            Err(err) => err,
                        };
                    }
                    ui.label(format!("Result: {}", self.calculator_output));

                    ui.separator();
                    ui.heading("Create New Variable");
                    ui.horizontal(|ui| {
                        ui.label("Name:");
                        ui.text_edit_singleline(&mut self.new_variable_name);
                        ui.label("Value:");
                        ui.text_edit_singleline(&mut self.new_variable_value);
                    });
                    if ui.button("Add Variable").clicked() {
                        self.user_variables.insert(self.new_variable_name.clone(), self.new_variable_value.clone());
                        self.new_variable_name.clear();
                        self.new_variable_value.clear();
                    }

                    let mut to_delete = Vec::new();
                    egui::ScrollArea::both().show(ui, |ui| {
                        TableBuilder::new(ui)
                            .striped(true)
                            .resizable(true)
                            .column(Column::initial(150.0).resizable(true))
                            .column(Column::remainder().resizable(true))
                            .column(Column::initial(60.0).resizable(false))
                            .header(20.0, |mut header| {
                                header.col(|ui| {
                                    ui.heading("Variable Name");
                                });
                                header.col(|ui| {
                                    ui.heading("Value");
                                });
                                header.col(|ui| {
                                    ui.heading("");
                                });
                            })
                            .body(|mut body| {
                                for (name, value) in &self.user_variables {
                                    body.row(20.0, |mut row| {
                                        row.col(|ui| {
                                            ui.label(name);
                                        });
                                        row.col(|ui| {
                                            ui.label(value);
                                        });
                                        row.col(|ui| {
                                            if ui.button("Delete").clicked() {
                                                to_delete.push(name.clone());
                                            }
                                        });
                                    });
                                }
                            });
                    });

                    for name in to_delete {
                        self.delete_user_variable(&name);
                    }
                }
            }
        });
    }
}
