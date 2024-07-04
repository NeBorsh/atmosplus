use copypasta::{ClipboardContext, ClipboardProvider};
use egui::{CentralPanel, Context, TopBottomPanel};
use egui_extras::{Column, TableBuilder};
use evalexpr::*;
use regex::Regex;
use reqwest;
use serde::Deserialize;
use serde_yaml::Value;
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
    gases: Vec<Gas>,
    reactions: Vec<Reaction>,
    selected_tab: Tab,
    sort_order: SortOrder,
    calculator_input: String,
    calculator_output: String,
    search_query: String,
    new_variable_name: String,
    new_variable_value: String,
    gases_loaded: bool,
    reactions_loaded: bool,
}

#[derive(PartialEq)]
enum Tab {
    ParseConstants,
    Calculator,
    Gases,
    Reactions,
}

#[derive(PartialEq)]
enum SortOrder {
    Ascending,
    Descending,
}

#[derive(Deserialize, Debug, Clone)]
struct Gas {
    name: String,
    #[serde(rename = "specificHeat")]
    specific_heat: Option<f64>,
    #[serde(rename = "heatCapacityRatio")]
    heat_capacity_ratio: Option<f64>,
    #[serde(rename = "molarMass")]
    molar_mass: Option<f64>,
}

#[derive(Deserialize, Debug, Clone)]
struct Reaction {
    id: String,
    priority: Option<i32>,
    #[serde(rename = "minimumTemperature")]
    minimum_temperature: Option<f64>,
    #[serde(rename = "maximumTemperature")]
    maximum_temperature: Option<f64>,
    #[serde(rename = "minimumRequirements")]
    minimum_requirements: Vec<f64>,
    effects: Vec<String>,
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

    fn fetch_and_parse_gases(&mut self) {
        let url = "https://raw.githubusercontent.com/space-wizards/space-station-14/master/Resources/Prototypes/Atmospherics/gases.yml";
        match reqwest::blocking::get(url) {
            Ok(response) => {
                if let Ok(text) = response.text() {
                    let docs: Vec<Value> = serde_yaml::from_str(&text).unwrap();
                    self.gases.clear();
                    for doc in docs {
                        let gas: Gas = serde_yaml::from_value(doc).unwrap_or(Gas {
                            name: "n/a".to_string(),
                            specific_heat: None,
                            heat_capacity_ratio: None,
                            molar_mass: None,
                        });
                        self.gases.push(gas);
                    }
                    self.gases_loaded = true;
                }
            }
            Err(err) => {
                eprintln!("Error fetching file: {:?}", err);
            }
        }
    }

    fn fetch_and_parse_reactions(&mut self) {
        let url = "https://raw.githubusercontent.com/space-wizards/space-station-14/master/Resources/Prototypes/Atmospherics/reactions.yml";
        match reqwest::blocking::get(url) {
            Ok(response) => {
                if let Ok(text) = response.text() {
                    let docs: Vec<Value> = serde_yaml::from_str(&text).unwrap();
                    self.reactions.clear();
                    for doc in docs {
                        if let Value::Mapping(mut map) = doc {
                            let id = map.get(&Value::String("id".to_string())).and_then(Value::as_str).unwrap_or("n/a").to_string();
                            let priority = map.get(&Value::String("priority".to_string())).and_then(Value::as_i64).map(|v| v as i32);
                            let minimum_temperature = map.get(&Value::String("minimumTemperature".to_string())).and_then(Value::as_f64);
                            let maximum_temperature = map.get(&Value::String("maximumTemperature".to_string())).and_then(Value::as_f64);
                            let minimum_requirements = map.get(&Value::String("minimumRequirements".to_string()))
                                .and_then(Value::as_sequence)
                                .map(|seq| seq.iter().filter_map(Value::as_f64).collect())
                                .unwrap_or_else(Vec::new);
                            
                            let effects = if let Some(Value::Sequence(effects)) = map.remove(&Value::String("effects".to_string())) {
                                MyApp::parse_effects(effects)
                            } else {
                                vec!["n/a".to_string()]
                            };
    
                            self.reactions.push(Reaction {
                                id,
                                priority,
                                minimum_temperature,
                                maximum_temperature,
                                minimum_requirements,
                                effects,
                            });
                        } else {
                            eprintln!("Failed to parse reaction: invalid document format");
                        }
                    }
                    self.reactions_loaded = true;
                }
            }
            Err(err) => {
                eprintln!("Error fetching file: {:?}", err);
            }
        }
    }

    fn parse_effects(effects: Vec<Value>) -> Vec<String> {
        effects.into_iter().map(|effect| {
            if let Value::Tagged(tagged_value) = effect {
                format!("{}{}", tagged_value.tag, serde_yaml::to_string(&tagged_value.value).unwrap_or_default())
            } else {
                serde_yaml::to_string(&effect).unwrap_or_default()
            }
        }).collect()
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
        if let Ok(mut ctx) = ClipboardContext::new() {
            let mut clipboard_content = String::new();
            for (name, value) in &self.filtered_constants {
                clipboard_content.push_str(&format!("{}\t{}\n", name, value));
            }
            if let Err(err) = ctx.set_contents(clipboard_content) {
                eprintln!("Error copying to clipboard: {:?}", err);
            }
        } else {
            eprintln!("Error creating clipboard context.");
        }
    }

    fn evaluate_expression(&self, expression: &str, context: &HashMapContext) -> Result<String, String> {
        match eval_with_context(expression, context) {
            Ok(value) => Ok(format!("{}", value)),
            Err(err) => Err(format!("Ошибка при вычислении: {}", err)),
        }
    }

    fn create_evaluation_context(&self) -> HashMapContext {
        let mut context = HashMapContext::new();
        self.add_variables_to_context(&mut context, &self.constants);
        self.add_variables_to_context(&mut context, &self.user_variables);
        context
    }

    fn add_variables_to_context(&self, context: &mut HashMapContext, variables: &HashMap<String, String>) {
        for (name, value) in variables {
            let cleaned_value = value.trim_end_matches('f');
            if let Ok(parsed_value) = cleaned_value.parse::<f64>() {
                context.set_value(name.clone(), evalexpr::Value::Float(parsed_value)).unwrap();
            } else {
                if let Ok(resolved_value) = self.resolve_expression(value, &mut context.clone()) {
                    if let Ok(parsed_value) = resolved_value.parse::<f64>() {
                        context.set_value(name.clone(), evalexpr::Value::Float(parsed_value)).unwrap();
                    }
                }
            }
        }
    }

    fn resolve_expression(&self, expression: &str, context: &mut HashMapContext) -> Result<String, String> {
        let mut expr = expression.to_string();
        let number_with_f_regex = Regex::new(r"(\d+\.?\d*e[-+]?\d*|\d+\.?\d*)f").unwrap();
        expr = number_with_f_regex.replace_all(&expr, "$1").to_string();

        let mut unresolved = true;
        while unresolved {
            unresolved = false;
            unresolved |= self.replace_variables_in_expression(&mut expr, &self.constants);
            unresolved |= self.replace_variables_in_expression(&mut expr, &self.user_variables);
        }

        self.evaluate_expression(&expr, context)
    }

    fn replace_variables_in_expression(&self, expr: &mut String, variables: &HashMap<String, String>) -> bool {
        let mut unresolved = false;
        for (name, value) in variables {
            let re = Regex::new(&format!(r"\b{}\b", regex::escape(name))).unwrap();
            if re.is_match(&expr) {
                unresolved = true;
                let resolved_value = if let Ok(parsed_value) = value.trim_end_matches('f').parse::<f64>() {
                    parsed_value.to_string()
                } else {
                    self.resolve_expression(value, &mut HashMapContext::new()).unwrap_or_else(|_| value.clone())
                };
                *expr = re.replace_all(&expr, resolved_value.as_str()).to_string();
            }
        }
        unresolved
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
                if ui.selectable_label(self.selected_tab == Tab::Gases, "Gases").clicked() {
                    self.selected_tab = Tab::Gases;
                    if !self.gases_loaded {
                        self.fetch_and_parse_gases();
                    }
                }
                if ui.selectable_label(self.selected_tab == Tab::Reactions, "Reactions").clicked() {
                    self.selected_tab = Tab::Reactions;
                    if !self.reactions_loaded {
                        self.fetch_and_parse_reactions();
                    }
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
                Tab::Gases => {
                    ui.heading("Gases");
                    if ui.button("Load Gases").clicked() {
                        self.fetch_and_parse_gases();
                    }
                    egui::ScrollArea::both().show(ui, |ui| {
                        TableBuilder::new(ui)
                            .striped(true)
                            .resizable(true)
                            .column(Column::initial(200.0).resizable(true))
                            .column(Column::initial(100.0).resizable(true))
                            .column(Column::initial(100.0).resizable(true))
                            .column(Column::initial(100.0).resizable(true))
                            .header(20.0, |mut header| {
                                header.col(|ui| {
                                    ui.heading("Gas Name");
                                });
                                header.col(|ui| {
                                    ui.heading("Specific Heat");
                                });
                                header.col(|ui| {
                                    ui.heading("Heat Capacity Ratio");
                                });
                                header.col(|ui| {
                                    ui.heading("Molar Mass");
                                });
                            })
                            .body(|mut body| {
                                for gas in &self.gases {
                                    body.row(20.0, |mut row| {
                                        row.col(|ui| {
                                            ui.label(&gas.name);
                                        });
                                        row.col(|ui| {
                                            ui.label(gas.specific_heat.map_or("n/a".to_string(), |v| v.to_string()));
                                        });
                                        row.col(|ui| {
                                            ui.label(gas.heat_capacity_ratio.map_or("n/a".to_string(), |v| v.to_string()));
                                        });
                                        row.col(|ui| {
                                            ui.label(gas.molar_mass.map_or("n/a".to_string(), |v| v.to_string()));
                                        });
                                    });
                                }
                            });
                    });
                }
                Tab::Reactions => {
                    ui.heading("Reactions");
                    if ui.button("Load Reactions").clicked() {
                        self.fetch_and_parse_reactions();
                    }
                    egui::ScrollArea::both().show(ui, |ui| {
                        TableBuilder::new(ui)
                            .striped(true)
                            .resizable(true)
                            .column(Column::initial(200.0).resizable(true))
                            .column(Column::initial(60.0).resizable(true))
                            .column(Column::initial(100.0).resizable(true))
                            .column(Column::initial(100.0).resizable(true))
                            .column(Column::initial(200.0).resizable(true))
                            .column(Column::initial(100.0).resizable(true))
                            .header(20.0, |mut header| {
                                header.col(|ui| {
                                    ui.heading("Reaction ID");
                                });
                                header.col(|ui| {
                                    ui.heading("Priority");
                                });
                                header.col(|ui| {
                                    ui.heading("Minimum Temperature");
                                });
                                header.col(|ui| {
                                    ui.heading("Maximum Temperature");
                                });
                                header.col(|ui| {
                                    ui.heading("Minimum Requirements");
                                });
                                header.col(|ui| {
                                    ui.heading("Effects");
                                });
                            })
                            .body(|mut body| {
                                for reaction in &self.reactions {
                                    body.row(20.0, |mut row| {
                                        row.col(|ui| {
                                            ui.label(&reaction.id);
                                        });
                                        row.col(|ui| {
                                            ui.label(reaction.priority.map_or("n/a".to_string(), |v| v.to_string()));
                                        });
                                        row.col(|ui| {
                                            ui.label(reaction.minimum_temperature.map_or("n/a".to_string(), |v| v.to_string()));
                                        });
                                        row.col(|ui| {
                                            ui.label(reaction.maximum_temperature.map_or("n/a".to_string(), |v| v.to_string()));
                                        });
                                        row.col(|ui| {
                                            ui.label(format!("{:?}", reaction.minimum_requirements));
                                        });
                                        row.col(|ui| {
                                            ui.label(format!("{:?}", reaction.effects));
                                        });
                                    });
                                }
                            });
                            ui.separator();
                            ui.label("Minimum Requirements Index Table:\n1)oxygen\n2)nitrogen\n3)carbon dioxide\n4)plasma\n5)tritium\n6)vapor\n7)ammonia\n8)n2o\n9)frezon");
                    });
                }
            }
        });
    }
}
