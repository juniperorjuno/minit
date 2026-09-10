#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use chrono::{DateTime, Utc};
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
enum Priority {
    Low,
    Medium,
    High,
}

impl Priority {
    fn label(&self) -> &'static str {
        match self {
            Priority::Low => "Low",
            Priority::Medium => "Medium",
            Priority::High => "High",
        }
    }

    fn color(&self) -> egui::Color32 {
        match self {
            Priority::Low => egui::Color32::from_rgb(96, 165, 250),
            Priority::Medium => egui::Color32::from_rgb(250, 204, 21),
            Priority::High => egui::Color32::from_rgb(248, 113, 113),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct Task {
    id: Uuid,
    title: String,
    done: bool,
    priority: Priority,
    estimated_minutes: u32,
    created_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    skipped_until: Option<DateTime<Utc>>,
}

impl Task {
    fn new(title: String, priority: Priority, estimated_minutes: u32) -> Self {
        Self {
            id: Uuid::new_v4(),
            title,
            done: false,
            priority,
            estimated_minutes,
            created_at: Utc::now(),
            completed_at: None,
            skipped_until: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Active,
    Completed,
}

enum Suggestion {
    Fits(Uuid),
    FitsSkipped(Uuid),
    Fallback(Uuid),
    NoTasks,
}

struct MinitApp {
    tasks: Vec<Task>,
    new_task_text: String,
    new_task_priority: Priority,
    new_task_minutes: u32,
    available_minutes: u32,
    filter: Filter,
    editing_id: Option<Uuid>,
    edit_buffer: String,
    save_path: PathBuf,
}

impl MinitApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let save_path = Self::data_file_path();
        let tasks = Self::load(&save_path);
        Self {
            tasks,
            new_task_text: String::new(),
            new_task_priority: Priority::Medium,
            new_task_minutes: 15,
            available_minutes: 15,
            filter: Filter::All,
            editing_id: None,
            edit_buffer: String::new(),
            save_path,
        }
    }

    fn data_file_path() -> PathBuf {
        if let Some(proj_dirs) = directories::ProjectDirs::from("", "", "minit") {
            let dir = proj_dirs.data_dir();
            let _ = fs::create_dir_all(dir);
            dir.join("tasks.json")
        } else {
            PathBuf::from("tasks.json")
        }
    }
    
    fn load(path: &PathBuf) -> Vec<Task> {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_tasks(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.tasks) {
            let tmp_path = self.save_path.with_extension("json.tmp");
            if fs::write(&tmp_path, json).is_ok() {
                let _ = fs::rename(&tmp_path, &self.save_path);
            }
        }
    }

    fn add_task(&mut self) {
        let title = self.new_task_text.trim().to_string();
        if !title.is_empty() {
            self.tasks.push(Task::new(
                title,
                self.new_task_priority,
                self.new_task_minutes.max(1),
            ));
            self.new_task_text.clear();
            self.save_tasks();
        }
    }

    fn delete_task(&mut self, id: Uuid) {
        self.tasks.retain(|t| t.id != id);
        self.save_tasks();
    }

    fn skip_task(&mut self, id: Uuid) {
        if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
            t.skipped_until = Some(Utc::now() + chrono::Duration::minutes(self.available_minutes as i64));
        }
        self.save_tasks();
    }

    fn toggle_task(&mut self, id: Uuid) {
        if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
            t.done = !t.done;
            t.completed_at = if t.done { Some(Utc::now()) } else { None };
        }
        self.save_tasks();
    }

    fn clear_completed(&mut self) {
        self.tasks.retain(|t| !t.done);
        self.save_tasks();
    }

    fn visible_tasks(&self) -> Vec<&Task> {
        self.tasks
            .iter()
            .filter(|t| match self.filter {
                Filter::All => true,
                Filter::Active => !t.done,
                Filter::Completed => t.done,
            })
            .collect()
    }

    fn suggest(&self) -> Suggestion {
        let active: Vec<&Task> = self.tasks.iter().filter(|t| !t.done).collect();
        if active.is_empty() {
            return Suggestion::NoTasks;
        }

        let now = Utc::now();
        let not_skipped = |t: &&&Task| t.skipped_until.map_or(true, |until| until <= now);

        let fitting: Vec<&&Task> = active
            .iter()
            .filter(|t| t.estimated_minutes <= self.available_minutes)
            .collect();

        if fitting.is_empty() {
            let shortest = active
                .iter()
                .min_by_key(|t| (t.estimated_minutes, t.created_at))
                .unwrap();
            return Suggestion::Fallback(shortest.id);
        }

        let ranked_by_priority = |list: &mut Vec<&&Task>| {
            list.sort_by(|a, b| {
                b.priority
                    .cmp(&a.priority)
                    .then_with(|| {
                        let sub_a = self.available_minutes.saturating_sub(a.estimated_minutes);
                        let sub_b = self.available_minutes.saturating_sub(b.estimated_minutes);
                        sub_a.cmp(&sub_b)
                    })
                    .then_with(|| a.created_at.cmp(&b.created_at))
            });
        };

        let mut candidates: Vec<&&Task> = fitting.iter().copied().filter(not_skipped).collect();

        if !candidates.is_empty() {
            ranked_by_priority(&mut candidates);
            return Suggestion::Fits(candidates[0].id);
        }

        let shortest_fitting = fitting
            .iter()
            .min_by_key(|t| (t.estimated_minutes, t.created_at))
            .unwrap();
        Suggestion::FitsSkipped(shortest_fitting.id)
    }

    fn task_by_id(&self, id: Uuid) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }
}

impl eframe::App for MinitApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("suggestion_panel").show(ui, |ui| {
            ui.add_space(8.0);
            ui.heading("Minit");
            ui.label(egui::RichText::new("What can you get done right now?").weak());
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("I have");
                ui.add(
                    egui::DragValue::new(&mut self.available_minutes)
                    .range(1..=600)
                    .suffix(" min"),
                );
                ui.label("before I need to do something else.");
            });

            ui.add_space(8.0);

            egui::Frame::canvas(ui.style())
                .fill(ui.visuals().extreme_bg_color)
                .corner_radius(8.0)
                .inner_margin(12.0)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    match self.suggest() {
                        Suggestion::NoTasks => {
                            ui.label("No tasks yet. Add one below to get a suggestion.");
                        }
                        Suggestion::Fallback(id) => {
                            if let Some(t) = self.task_by_id(id) {
                                ui.label(
                                    egui::RichText::new("Nothing fits perfectly, but here's your shortest task:")
                                        .weak(),
                                );
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui
                                        .allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                                    ui.painter().circle_filled(rect.center(), 5.0, t.priority.color());
                                    ui.strong(&t.title);
                                    ui.label(format!(
                                        "({} · {} {} — needs {} more than you have)",
                                        t.priority.label(),
                                        t.estimated_minutes,
                                        if t.estimated_minutes == 1 { "min" } else { "mins" },
                                        t.estimated_minutes.saturating_sub(self.available_minutes)
                                    ));
                                });
                                if ui.button("Mark done").clicked() {
                                    self.toggle_task(id);
                                }
                                if ui.button("Now now >>").clicked() {
                                    self.skip_task(id);
                                }
                            }
                        }
                        Suggestion::Fits(id) => {
                            if let Some(t) = self.task_by_id(id) {
                                ui.label(egui::RichText::new("Right now, do this:").weak());
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui
                                        .allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                                    ui.painter().circle_filled(rect.center(), 5.0, t.priority.color());
                                    ui.heading(&t.title);
                                });
                                ui.label(format!(
                                    "{} priority · about {} {}",
                                    t.priority.label(),
                                    t.estimated_minutes,
                                    if t.estimated_minutes == 1 { "min" } else { "mins" }
                                ));
                                ui.horizontal(|ui| {
                                    if ui.button("✅ Mark done").clicked() {
                                        self.toggle_task(id);
                                    }
                                    if ui.button("⏭ Not now").clicked() {
                                        self.skip_task(id);
                                    }
                                });
                            }
                        }
                        Suggestion::FitsSkipped(id) => {
                            if let Some(t) = self.task_by_id(id) {
                                ui.label(egui::RichText::new("You skipped every task, here's the smallest thing you could still knock out:").weak());
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui
                                        .allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                                    ui.painter().circle_filled(rect.center(), 5.0, t.priority.color());
                                    ui.heading(&t.title);
                                });
                                ui.label(format!(
                                    "{} priority · about {} {}",
                                    t.priority.label(),
                                    t.estimated_minutes,
                                    if t.estimated_minutes == 1 { "min" } else { "mins" }
                                ));
                                ui.horizontal(|ui| {
                                    if ui.button("✅ Mark done").clicked() {
                                        self.toggle_task(id);
                                    }
                                    if ui.button("⏭ Not now").clicked() {
                                        self.skip_task(id);
                                    }
                                });
                            }
                        }
                    }
                });

            ui.add_space(10.0);
        });

        egui::Panel::bottom("footer_panel").show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.new_task_text)
                        .hint_text("New task…")
                        .desired_width(200.0),
                );
                ui.add(
                    egui::DragValue::new(&mut self.new_task_minutes)
                        .range(1..=600)
                        .suffix(" min"),
                );
                
                egui::ComboBox::from_id_salt("priority_picker")
                    .selected_text(self.new_task_priority.label())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.new_task_priority, Priority::Low, "Low");
                        ui.selectable_value(&mut self.new_task_priority, Priority::Medium, "Medium");
                        ui.selectable_value(&mut self.new_task_priority, Priority::High, "High");
                    });

                if ui.button("Add").clicked()
                    || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    self.add_task();
                    response.request_focus();
                }
            });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.filter, Filter::All, "All");
                ui.selectable_value(&mut self.filter, Filter::Active, "Active");
                ui.selectable_value(&mut self.filter, Filter::Completed, "Completed");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let remaining = self.tasks.iter().filter(|t| !t.done).count();
                    if ui.button("Clear completed").clicked() {
                        self.clear_completed();
                    }
                    ui.label(format!(
                        "{} item{} left",
                        remaining,
                        if remaining == 1 { "" } else { "s" }
                    ));
                });
            });
            ui.add_space(4.0);
        });

        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut to_delete: Option<Uuid> = None;
                let mut to_toggle: Option<Uuid> = None;

                let rows: Vec<(Uuid, bool, Priority, String, u32)> = self
                    .visible_tasks()
                    .into_iter()
                    .map(|t| (t.id, t.done, t.priority, t.title.clone(), t.estimated_minutes))
                    .collect();

                for (id, done_flag, priority, title, estimated_minutes) in rows {
                    ui.horizontal(|ui| {
                        let mut done = done_flag;
                        if ui.checkbox(&mut done, "").changed() {
                            to_toggle = Some(id);
                        }

                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                        ui.painter().circle_filled(rect.center(), 5.0, priority.color());

                       if self.editing_id == Some(id) {
                            let edit_id = ui.make_persistent_id("task_edit_field");
                            
                            let edit_response = ui.add(
                                egui::TextEdit::singleline(&mut self.edit_buffer)
                                    .id(edit_id)
                                    .desired_width(f32::INFINITY)
                            );

                            let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                            let escape_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));

                            if escape_pressed {
                                self.editing_id = None;
                            } else if enter_pressed || edit_response.lost_focus() {
                                let new_title = self.edit_buffer.trim().to_string();
                                if !new_title.is_empty() {
                                    if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
                                        t.title = new_title;
                                    }
                                    self.save_tasks();
                                }
                                self.editing_id = None;
                            }
                        } else {
                            let text = if done_flag {
                                egui::RichText::new(&title).strikethrough().weak()
                            } else {
                                egui::RichText::new(&title)
                            };

                            let label_response = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                            if label_response.double_clicked() {
                                self.editing_id = Some(id);
                                self.edit_buffer = title.clone();
                                
                                let edit_id = ui.make_persistent_id("task_edit_field");
                                ui.memory_mut(|m| m.request_focus(edit_id));
                            }
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("🗑").clicked() {
                                to_delete = Some(id);
                            }
                            ui.label(format!("{} min", estimated_minutes));
                        });
                    });
                    ui.separator();
                }

                if let Some(id) = to_toggle {
                    self.toggle_task(id);
                }
                if let Some(id) = to_delete {
                    self.delete_task(id);
                }
            });
        });
    }
}

fn main() -> eframe::Result<()> {
        let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_icon(std::sync::Arc::new(egui::IconData {
                rgba: image::load_from_memory(include_bytes!("../assets/icon.png"))
                    .unwrap()
                    .to_rgba8()
                    .to_vec(),
                width: 384,
                height: 384,
            }))
            .with_inner_size([440.0, 640.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Minit",
        options,
        Box::new(|cc| Ok(Box::new(MinitApp::new(cc)) as Box<dyn eframe::App>)),
    )
}