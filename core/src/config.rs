use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub version: u32,
    pub central_skills_path: String,
    pub targets: Vec<Target>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Target {
    pub id: u32,
    pub name: String,
    pub path: String,
    pub enabled: bool,
}

impl Default for Target {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            path: String::new(),
            enabled: true,
        }
    }
}

fn user_profile() -> String {
    std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".to_string())
}

pub fn builtin_targets() -> Vec<Target> {
    let up = user_profile();
    vec![
        Target { id: 1, name: "Claude Code".into(), path: format!("{up}\\.claude\\skills"), enabled: true },
        Target { id: 2, name: "CodeBuddy".into(), path: format!("{up}\\.codebuddy\\skills"), enabled: true },
        Target { id: 3, name: "Codex".into(), path: format!("{up}\\.codex\\skills"), enabled: true },
        Target { id: 4, name: "Cursor".into(), path: format!("{up}\\.cursor\\skills"), enabled: true },
        Target { id: 5, name: "Gemini CLI".into(), path: format!("{up}\\.gemini\\skills"), enabled: true },
        Target { id: 6, name: "OpenCode".into(), path: format!("{up}\\.config\\opencode\\skills"), enabled: true },
    ]
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            central_skills_path: String::new(), // 默认为空，首次启动引导用户设置
            targets: builtin_targets(),
        }
    }
}

pub fn config_path() -> PathBuf {
    PathBuf::from(user_profile()).join(".skill-client").join("config.json")
}

/// 读：文件不存在或解析失败 → 默认配置
pub fn load_from(path: &Path) -> Config {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn load() -> Config {
    load_from(&config_path())
}

/// 写：新增 id 为 0/缺失的 target 由后端分配 max+1；pretty camelCase 落盘
pub fn save_to(path: &Path, cfg: &mut Config) -> Result<(), String> {
    let mut max_id = cfg.targets.iter().map(|t| t.id).max().unwrap_or(0);
    for t in cfg.targets.iter_mut() {
        if t.id == 0 {
            max_id += 1;
            t.id = max_id;
        }
    }
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, json).map_err(|e| e.to_string())
}

pub fn save(cfg: &mut Config) -> Result<(), String> {
    save_to(&config_path(), cfg)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "skill-link-cfg-{tag}-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn should_load_config_from_disk() {
        let dir = temp_dir("load");
        let file = dir.join("config.json");
        std::fs::write(
            &file,
            r#"{"version":1,"centralSkillsPath":"D:/myskills","targets":[{"id":1,"name":"Claude Code","path":"C:/u/.claude/skills","enabled":false}]}"#,
        )
        .unwrap();
        let cfg = load_from(&file);
        assert_eq!(cfg.version, 1);
        assert_eq!(cfg.central_skills_path, "D:/myskills");
        assert_eq!(cfg.targets.len(), 1);
        let t = &cfg.targets[0];
        assert_eq!((t.id, t.name.as_str(), t.path.as_str(), t.enabled), (1, "Claude Code", "C:/u/.claude/skills", false));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn should_return_defaults_when_file_missing() {
        let dir = temp_dir("missing");
        let cfg = load_from(&dir.join("config.json"));
        assert_eq!(cfg.version, 1);
        assert_eq!(cfg.targets.len(), 6);
        assert_eq!(cfg.targets.iter().map(|t| t.id).collect::<Vec<_>>(), vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(cfg.targets[0].name, "Claude Code");
        assert_eq!(cfg.targets[5].name, "OpenCode");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn should_fallback_to_defaults_on_broken_json() {
        let dir = temp_dir("broken");
        let file = dir.join("config.json");
        std::fs::write(&file, "{ not valid json !!!").unwrap();
        let cfg = load_from(&file);
        assert_eq!(cfg, Config::default());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn should_merge_missing_fields_with_defaults() {
        let dir = temp_dir("merge");
        let file = dir.join("config.json");
        // 缺 centralSkillsPath，但 targets 自定义
        std::fs::write(
            &file,
            r#"{"version":1,"targets":[{"id":9,"name":"My Tool","path":"X:/t","enabled":true}]}"#,
        )
        .unwrap();
        let cfg = load_from(&file);
        assert_eq!(cfg.central_skills_path, Config::default().central_skills_path);
        assert_eq!(cfg.targets.len(), 1);
        assert_eq!(cfg.targets[0].id, 9);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn should_assign_next_id_on_save() {
        let dir = temp_dir("assign");
        let file = dir.join("config.json");
        let mut cfg = Config {
            targets: vec![
                Target { id: 6, name: "A".into(), path: "P:/a".into(), enabled: true },
                Target { id: 0, name: "New One".into(), path: "P:/n".into(), enabled: true },
            ],
            ..Default::default()
        };
        save_to(&file, &mut cfg).unwrap();
        let reloaded = load_from(&file);
        let new_one = reloaded.targets.iter().find(|t| t.name == "New One").unwrap();
        assert_eq!(new_one.id, 7);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn should_save_pretty_camel_case_json() {
        let dir = temp_dir("pretty");
        let file = dir.join("config.json");
        let mut cfg = Config::default();
        save_to(&file, &mut cfg).unwrap();
        let raw = std::fs::read_to_string(&file).unwrap();
        assert!(raw.contains("\"centralSkillsPath\""), "camelCase 键名缺失：{raw}");
        assert!(!raw.contains("central_skills_path"));
        let reloaded = load_from(&file);
        assert_eq!(reloaded.version, 1);
        assert_eq!(reloaded.targets[0].id, 1);
        std::fs::remove_dir_all(&dir).ok();
    }
}
