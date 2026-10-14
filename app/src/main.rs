// Windows 下使用 GUI 子系统：双击启动不弹出控制台黑窗
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use slint::{ComponentHandle, Model, ModelRc, VecModel};
use skill_link_core::config::{self, Config, Target};
use skill_link_core::links::{self, LinkStatus};
use skill_link_core::skills::{self, SkillInfo};
use std::cell::RefCell;
use std::rc::Rc;

slint::include_modules!();

#[derive(Clone)]
struct App {
    ui: slint::Weak<MainWindow>,
    /// 冲突待执行部署：(skill, checked target ids)
    pending: Rc<RefCell<Option<(String, Vec<u32>)>>>,
}

impl App {
    fn cfg(&self) -> Config {
        config::load()
    }

    fn toast(&self, msg: &str, error: bool) {
        let ui = self.ui.upgrade().expect("ui dropped");
        ui.set_toast(msg.into());
        ui.set_toast_error(error);
    }

    fn set_deploy_rows(&self, rows: Vec<DeployRow>) {
        let ui = self.ui.upgrade().expect("ui dropped");
        ui.set_deploy_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
    }

    /// 读取详情页勾选的 target ids
    fn checked_ids(&self) -> Vec<u32> {
        let ui = self.ui.upgrade().expect("ui dropped");
        ui.get_deploy_rows()
            .iter()
            .filter(|r| r.checked)
            .map(|r| r.id as u32)
            .collect()
    }

    /// 从 UI targets 模型读取配置行
    fn targets_from_ui(&self) -> Vec<Target> {
        let ui = self.ui.upgrade().expect("ui dropped");
        ui.get_targets()
            .iter()
            .map(|t| Target {
                id: t.id as u32,
                name: t.name.to_string(),
                path: t.path.to_string(),
                enabled: t.enabled,
            })
            .collect()
    }

    /// 全量刷新（保留按 dirName 的选中态，与 Web 版一致）
    fn refresh(&self) {
        let ui = self.ui.upgrade().expect("ui dropped");
        let cfg = self.cfg();
        let central = cfg.central_skills_path.clone();

        // 记住原选中 skill
        let prev_len = ui.get_skills().row_count();
        let prev_sel = ui.get_selected_index();
        let prev_dir = if prev_sel >= 0 && (prev_sel as usize) < prev_len {
            Some(ui.get_skills().row_data(prev_sel as usize).unwrap().dir_name.to_string())
        } else {
            None
        };

        let targets_vm: Vec<TargetItem> = cfg
            .targets
            .iter()
            .map(|t| TargetItem {
                id: t.id as i32,
                name: t.name.clone().into(),
                path: t.path.clone().into(),
                enabled: t.enabled,
            })
            .collect();

        let list = skills::list_skills(&central);
        let skills_vm: Vec<SkillRow> = list
            .iter()
            .map(|s| {
                // 徽章每行 3 个（模拟 flex-wrap）
                let badges: Vec<StatusBadge> = cfg
                    .targets
                    .iter()
                    .map(|t| StatusBadge {
                        target_name: t.name.clone().into(),
                        label: status_label(links::status_of(&t.path, &central, &s.dir_name)).into(),
                    })
                    .collect();
                let badge_rows: Vec<StatusRow> = badges
                    .chunks(3)
                    .map(|chunk| StatusRow {
                        items: ModelRc::from(Rc::new(VecModel::from(chunk.to_vec()))),
                    })
                    .collect();
                SkillRow {
                    dir_name: s.dir_name.clone().into(),
                    name: s.name.clone().into(),
                    initial: s.name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default().into(),
                    description: s.description.clone().into(),
                    badge_rows: ModelRc::from(Rc::new(VecModel::from(badge_rows))),
                }
            })
            .collect();

        // 恢复选中
        let new_sel = prev_dir
            .and_then(|d| list.iter().position(|s| s.dir_name == d))
            .or(if list.is_empty() { None } else { Some(0) })
            .map(|i| i as i32)
            .unwrap_or(-1);

        ui.set_targets(ModelRc::from(Rc::new(VecModel::from(targets_vm))));
        ui.set_skills(ModelRc::from(Rc::new(VecModel::from(skills_vm))));
        ui.set_central_path(central.clone().into());
        ui.set_selected_index(new_sel);

        // 顶部统计：全部 target × 全部 skill 的状态汇总
        let mut linked = 0u32;
        let mut conflict = 0u32;
        let mut broken = 0u32;
        for s in &list {
            for t in &cfg.targets {
                match links::status_of(&t.path, &central, &s.dir_name) {
                    LinkStatus::Linked => linked += 1,
                    LinkStatus::Conflict => conflict += 1,
                    LinkStatus::Broken => broken += 1,
                    LinkStatus::NotLinked => {}
                }
            }
        }
        ui.set_stats(Stats {
            total: list.len() as i32,
            linked: linked as i32,
            conflict: conflict as i32,
            broken: broken as i32,
        });

        self.refresh_deploy_rows();
    }

    /// 刷新详情页部署目标勾选列表（仅启用 target，默认全勾选）
    fn refresh_deploy_rows(&self) {
        let ui = self.ui.upgrade().expect("ui dropped");
        let cfg = self.cfg();
        let central = cfg.central_skills_path.clone();
        let sel = ui.get_selected_index();
        let rows: Vec<DeployRow> = if sel < 0 {
            Vec::new()
        } else {
            let skills = ui.get_skills();
            let Some(s) = skills.row_data(sel as usize) else {
                return;
            };
            let dir = s.dir_name.to_string();
            cfg.targets
                .iter()
                .filter(|t| t.enabled)
                .map(|t| DeployRow {
                    id: t.id as i32,
                    name: t.name.clone().into(),
                    checked: true,
                    status_label: status_label(links::status_of(&t.path, &central, &dir)).into(),
                })
                .collect()
        };
        self.set_deploy_rows(rows);
    }
}

fn status_label(s: LinkStatus) -> &'static str {
    match s {
        LinkStatus::NotLinked => "NotLinked",
        LinkStatus::Linked => "Linked",
        LinkStatus::Conflict => "Conflict",
        LinkStatus::Broken => "Broken",
    }
}

fn find_target_by_id(cfg: &Config, id: u32) -> Result<Target, String> {
    cfg.targets
        .iter()
        .find(|t| t.id == id)
        .cloned()
        .ok_or_else(|| format!("target 不存在: id={id}"))
}

fn report(app: &App, errors: &[String], ok_msg: &str) {
    if errors.is_empty() {
        app.toast(ok_msg, false);
    } else {
        let extra = if errors.len() > 1 { " …" } else { "" };
        app.toast(&format!("失败 {} 项：{}{}", errors.len(), errors[0], extra), true);
    }
    app.refresh();
}

fn main() {
    // 使用软件渲染器，文字抗锯齿质量更好（Skia 需 clang 工具链，暂不用）
    // main 启动早期、单线程调用，安全
    unsafe { std::env::set_var("SLINT_BACKEND", "winit-software") };
    let ui = MainWindow::new().expect("创建窗口失败");
    let app = App { ui: ui.as_weak(), pending: Rc::new(RefCell::new(None)) };

    ui.on_reload({
        let app = app.clone();
        move || app.refresh()
    });

    ui.on_select_skill({
        let app = app.clone();
        move |idx| {
            let ui = app.ui.upgrade().expect("ui dropped");
            ui.set_selected_index(idx);
            app.refresh_deploy_rows();
        }
    });

    ui.on_deploy_all_skills({
        let app = app.clone();
        move || {
            let cfg = app.cfg();
            let names: Vec<String> = skills::list_skills(&cfg.central_skills_path)
                .into_iter()
                .map(|s: SkillInfo| s.dir_name)
                .collect();
            let targets: Vec<(String, String, bool)> = cfg
                .targets
                .iter()
                .map(|t| (t.name.clone(), t.path.clone(), t.enabled))
                .collect();
            let (ok, errors) = links::link_all_skills(&cfg.central_skills_path, &names, &targets);
            if errors.is_empty() {
                app.toast(&format!("全部部署完成：成功 {ok} 项"), false);
                app.refresh();
            } else {
                let (s, t, e) = &errors[0];
                let extra = if errors.len() > 1 { " …" } else { "" };
                app.toast(
                    &format!("全部部署：成功 {ok} 项，失败 {} 项（如 {s} × {t}: {e}）{}", errors.len(), extra),
                    true,
                );
                app.refresh();
            }
        }
    });

    ui.on_deploy_selected({
        let app = app.clone();
        move |skill| {
            let cfg = app.cfg();
            let ids = app.checked_ids();
            if ids.is_empty() {
                app.toast("请勾选至少一个 target", true);
                return;
            }
            let has_conflict = {
                let ui = app.ui.upgrade().expect("ui dropped");
                ui.get_deploy_rows()
                    .iter()
                    .any(|r| r.checked && r.status_label == "Conflict")
            };
            if has_conflict {
                // 弹窗选择处理方式，暂存待执行
                *app.pending.borrow_mut() = Some((skill.to_string(), ids.clone()));
                let ui = app.ui.upgrade().expect("ui dropped");
                ui.set_conflict_open(true);
                return;
            }
            let errors: Vec<String> = ids
                .iter()
                .filter_map(|id| {
                    find_target_by_id(&cfg, *id)
                        .and_then(|t| links::link(&t.path, &cfg.central_skills_path, &skill, true))
                        .err()
                })
                .collect();
            report(&app, &errors, "部署完成");
        }
    });

    ui.on_conflict_choose({
        let app = app.clone();
        move |choice| {
            let ui = app.ui.upgrade().expect("ui dropped");
            ui.set_conflict_open(false);
            let Some((skill, ids)) = app.pending.borrow_mut().take() else { return };
            let backup = choice == 0;
            let cfg = app.cfg();
            let errors: Vec<String> = ids
                .iter()
                .filter_map(|id| {
                    find_target_by_id(&cfg, *id)
                        .and_then(|t| links::link(&t.path, &cfg.central_skills_path, &skill, backup))
                        .err()
                })
                .collect();
            report(&app, &errors, "部署完成");
        }
    });

    ui.on_remove_selected({
        let app = app.clone();
        move |skill| {
            let cfg = app.cfg();
            let ids = app.checked_ids();
            if ids.is_empty() {
                app.toast("请勾选至少一个 target", true);
                return;
            }
            let errors: Vec<String> = ids
                .iter()
                .filter_map(|id| {
                    find_target_by_id(&cfg, *id)
                        .and_then(|t| links::unlink(&t.path, &skill))
                        .err()
                })
                .collect();
            report(&app, &errors, "移除完成");
        }
    });

    ui.on_toggle_check({
        let app = app.clone();
        move |id, checked| {
            let ui = app.ui.upgrade().expect("ui dropped");
            let model = ui.get_deploy_rows();
            let mut rows: Vec<DeployRow> = model.iter().collect();
            for r in rows.iter_mut() {
                if r.id == id {
                    r.checked = checked;
                }
            }
            app.set_deploy_rows(rows);
        }
    });

    ui.on_pick_folder({
        let app = app.clone();
        move || {
            if let Some(dir) = rfd::FileDialog::new()
                .set_title("选择 skill 仓库路径")
                .pick_folder()
            {
                let path = dir.to_string_lossy().to_string();
                let ui = app.ui.upgrade().expect("ui dropped");
                ui.set_central_path(path.into());
                app.toast("已选择路径，点「保存路径」生效", false);
            }
        }
    });

    ui.on_save_central({
        let app = app.clone();
        move |path| {
            let path = path.trim().to_string();
            if path.is_empty() {
                app.toast("路径不能为空", true);
                return;
            }
            let mut cfg = app.cfg();
            cfg.central_skills_path = path;
            let r = config::save(&mut cfg);
            app.refresh();
            match r {
                Ok(()) => app.toast("配置已保存", false),
                Err(e) => app.toast(&format!("保存失败: {e}"), true),
            }
        }
    });

    ui.on_add_target({
        let app = app.clone();
        move || {
            let ui = app.ui.upgrade().expect("ui dropped");
            let mut items: Vec<TargetItem> = ui.get_targets().iter().collect();
            items.push(TargetItem {
                id: 0,
                name: "新 Target".into(),
                path: "".into(),
                enabled: true,
            });
            ui.set_targets(ModelRc::from(Rc::new(VecModel::from(items))));
            app.toast("已添加，点「保存配置」写回（id 自动分配）", false);
        }
    });

    ui.on_delete_target({
        let app = app.clone();
        move |id| {
            let ui = app.ui.upgrade().expect("ui dropped");
            let mut items: Vec<TargetItem> = ui.get_targets().iter().collect();
            items.retain(|t| t.id != id);
            ui.set_targets(ModelRc::from(Rc::new(VecModel::from(items))));
        }
    });

    ui.on_save_targets({
        let app = app.clone();
        move || {
            let mut cfg = app.cfg();
            // 保存 targets 前先收割中央仓库路径输入框（单一真源防覆盖）
            let ui = app.ui.upgrade().expect("ui dropped");
            let central = ui.get_central_path().trim().to_string();
            if !central.is_empty() {
                cfg.central_skills_path = central;
            }
            cfg.targets = app.targets_from_ui();
            let r = config::save(&mut cfg);
            app.refresh();
            match r {
                Ok(()) => app.toast("配置已保存", false),
                Err(e) => app.toast(&format!("保存失败: {e}"), true),
            }
        }
    });

    ui.on_set_target_name({
        let app = app.clone();
        move |id, v| update_target(&app, id, TargetField::Name(v.to_string()))
    });

    ui.on_set_target_path({
        let app = app.clone();
        move |id, v| update_target(&app, id, TargetField::Path(v.to_string()))
    });

    ui.on_toggle_target_enabled({
        let app = app.clone();
        move |id, v| update_target(&app, id, TargetField::Enabled(v))
    });

    app.refresh();
    ui.run().expect("运行失败");
}

enum TargetField {
    Name(String),
    Path(String),
    Enabled(bool),
}

fn update_target(app: &App, id: i32, field: TargetField) {
    let ui = app.ui.upgrade().expect("ui dropped");
    let mut items: Vec<TargetItem> = ui.get_targets().iter().collect();
    for t in items.iter_mut() {
        if t.id == id {
            match &field {
                TargetField::Name(v) => t.name = v.clone().into(),
                TargetField::Path(v) => t.path = v.clone().into(),
                TargetField::Enabled(v) => t.enabled = *v,
            }
        }
    }
    ui.set_targets(ModelRc::from(Rc::new(VecModel::from(items))));
}
