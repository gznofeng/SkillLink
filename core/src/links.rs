use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LinkStatus {
    NotLinked,
    Linked,
    Conflict,
    Broken,
}

/// 四态检测（只读）：A+C 方案
/// is_symlink 判链接类型，canonicalize 比对指向；指向非本仓库对应 skill 的链接按 Broken 处理。
pub fn status_of(target_dir: &str, central_dir: &str, skill_dir: &str) -> LinkStatus {
    let dest = Path::new(target_dir).join(skill_dir);
    let ft = match std::fs::symlink_metadata(&dest) {
        Ok(m) => m.file_type(),
        Err(_) => return LinkStatus::NotLinked,
    };
    if !ft.is_symlink() {
        return LinkStatus::Conflict;
    }
    // 是链接：验证指向
    let central_skill = Path::new(central_dir).join(skill_dir);
    let (dest_canon, central_canon) = match (dest.canonicalize(), central_skill.canonicalize()) {
        (Ok(d), Ok(c)) => (d, c),
        _ => return LinkStatus::Broken,
    };
    if dest_canon == central_canon {
        LinkStatus::Linked
    } else {
        LinkStatus::Broken
    }
}

/// 创建 junction：cmd /c mklink /J <target> <central>（逐参数传参，引号由 std 保证）
/// cmd 中文输出为 GBK，失败信息按 GBK 解码避免乱码；父目录不存在则先创建
fn create_junction(target: &Path, central: &Path) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目标父目录失败: {e}"))?;
    }
    let output = Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(target.as_os_str())
        .arg(central.as_os_str())
        .output()
        .map_err(|e| format!("启动 mklink 失败: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!("mklink 失败: {}", decode_cmd_output(&output.stderr).trim()))
    }
}

/// cmd 控制台输出解码：优先 UTF-8，否则按 GBK（代码页 936）
fn decode_cmd_output(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => {
            let (decoded, _, _) = encoding_rs::GBK.decode(bytes);
            decoded.into_owned()
        }
    }
}

fn next_backup_path(dest: &Path) -> PathBuf {
    let mut s = dest.as_os_str().to_os_string();
    s.push(".bak");
    let first = PathBuf::from(s);
    if !first.exists() {
        return first;
    }
    let now = format_utc_timestamp();
    let mut s = dest.as_os_str().to_os_string();
    s.push(format!(".bak-{now}"));
    PathBuf::from(s)
}

/// yyyyMMdd-HHmmss（UTC，来自系统 epoch；仅用于备份名唯一性与人类可读性）
fn format_utc_timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // Howard Hinnant civil_from_days 算法：epoch day → y/m/d
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}{m:02}{d:02}-{h:02}{mi:02}{s:02}")
}

/// link 部署。conflict=backup（默认）时冲突目录改名 .bak；overwrite 时直接删除。
pub fn link(target_dir: &str, central_dir: &str, skill_dir: &str, backup_on_conflict: bool) -> Result<(), String> {
    let dest = Path::new(target_dir).join(skill_dir);
    let central_skill = Path::new(central_dir).join(skill_dir);
    if !central_skill.is_dir() {
        return Err(format!("中央仓库不存在该 skill: {}", central_skill.display()));
    }
    match std::fs::symlink_metadata(&dest) {
        Err(_) => {} // 不存在，直接建
        Ok(m) if m.file_type().is_symlink() => {
            // 已是链接：指向本仓库对应 skill 才幂等返回；悬空/外来链接先移除再重建
            let already_linked = matches!(
                (dest.canonicalize(), Path::new(central_dir).join(skill_dir).canonicalize()),
                (Ok(d), Ok(c)) if d == c
            );
            if already_linked {
                return Ok(());
            }
            std::fs::remove_dir(&dest).map_err(|e| format!("移除失效链接失败: {e}"))?;
        }
        Ok(_) => {
            // Conflict：真实目录/文件
            if backup_on_conflict {
                let bak = next_backup_path(&dest);
                std::fs::rename(&dest, &bak).map_err(|e| format!("备份失败: {e}"))?;
            } else {
                if dest.is_dir() {
                    std::fs::remove_dir_all(&dest).map_err(|e| format!("删除原目录失败: {e}"))?;
                } else {
                    std::fs::remove_file(&dest).map_err(|e| format!("删除原文件失败: {e}"))?;
                }
            }
        }
    }
    create_junction(&dest, &central_skill)
}

/// unlink：仅当目标是链接时删除链接本身；真实目录拒绝；中央仓库永不受影响。
pub fn unlink(target_dir: &str, skill_dir: &str) -> Result<(), String> {
    let dest = Path::new(target_dir).join(skill_dir);
    match std::fs::symlink_metadata(&dest) {
        Err(_) => Err(format!("目标不存在链接: {}", dest.display())),
        Ok(m) if m.file_type().is_symlink() => {
            std::fs::remove_dir(&dest).map_err(|e| format!("移除链接失败: {e}"))
        }
        Ok(_) => Err(format!("目标处是真实目录/文件，拒绝删除: {}", dest.display())),
    }
}

/// link_all：对单个 skill 部署到所有启用的 target，返回 (target_name, 结果)。
pub fn link_all(central_dir: &str, skill_dir: &str, targets: &[(String, String, bool)]) -> Vec<(String, Result<(), String>)> {
    targets
        .iter()
        .filter(|(_, _, enabled)| *enabled)
        .map(|(name, path, _)| (name.clone(), link(path, central_dir, skill_dir, true)))
        .collect()
}

/// link_all_skills：全部 skill × 全部启用 target，返回 (成功数, [(skill, target, 错误)])。
pub fn link_all_skills(central_dir: &str, skills: &[String], targets: &[(String, String, bool)]) -> (u32, Vec<(String, String, String)>) {
    let enabled: Vec<&(String, String, bool)> = targets.iter().filter(|(_, _, e)| *e).collect();
    let mut ok = 0u32;
    let mut errors = Vec::new();
    for skill in skills {
        for (tname, tpath, _) in &enabled {
            match link(tpath, central_dir, skill, true) {
                Ok(()) => ok += 1,
                Err(e) => errors.push((skill.clone(), tname.clone(), e)),
            }
        }
    }
    (ok, errors)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        // 目录名故意带空格，验证 mklink 引号处理
        let d = std::env::temp_dir().join(format!(
            "skill link test {tag} {}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn make_skill(central: &Path, skill: &str) {
        let p = central.join(skill);
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join("SKILL.md"), "---\nname: t\n---\n").unwrap();
        std::fs::write(p.join("extra.txt"), "payload").unwrap();
    }

    #[test]
    fn should_report_not_linked_when_target_missing() {
        let central = temp_dir("c1");
        let target = temp_dir("t1");
        make_skill(&central, "commit");
        assert_eq!(status_of(target.to_str().unwrap(), central.to_str().unwrap(), "commit"), LinkStatus::NotLinked);
        cleanup(&[&central, &target]);
    }

    #[test]
    fn should_report_linked_when_junction_points_to_central() {
        let central = temp_dir("c2");
        let target = temp_dir("t2");
        make_skill(&central, "commit");
        link(target.to_str().unwrap(), central.to_str().unwrap(), "commit", true).unwrap();
        assert_eq!(status_of(target.to_str().unwrap(), central.to_str().unwrap(), "commit"), LinkStatus::Linked);
        cleanup(&[&central, &target]);
    }

    #[test]
    fn should_report_broken_when_junction_dangling() {
        let central = temp_dir("c3");
        let target = temp_dir("t3");
        make_skill(&central, "commit");
        link(target.to_str().unwrap(), central.to_str().unwrap(), "commit", true).unwrap();
        // 删除中央仓库对应目录 → 链接悬空
        std::fs::remove_dir_all(central.join("commit")).unwrap();
        assert_eq!(status_of(target.to_str().unwrap(), central.to_str().unwrap(), "commit"), LinkStatus::Broken);
        cleanup(&[&central, &target]);
    }

    #[test]
    fn should_report_conflict_when_real_dir_exists() {
        let central = temp_dir("c4");
        let target = temp_dir("t4");
        make_skill(&central, "commit");
        std::fs::create_dir_all(target.join("commit")).unwrap();
        std::fs::write(target.join("commit").join("real.txt"), "real").unwrap();
        assert_eq!(status_of(target.to_str().unwrap(), central.to_str().unwrap(), "commit"), LinkStatus::Conflict);
        cleanup(&[&central, &target]);
    }

    #[test]
    fn should_treat_foreign_junction_as_broken() {
        let central = temp_dir("c5");
        let target = temp_dir("t5");
        let foreign = temp_dir("f5");
        make_skill(&central, "commit");
        std::fs::create_dir_all(foreign.join("elsewhere")).unwrap();
        // junction 指向第三方目录
        assert!(create_junction(&target.join("commit"), &foreign.join("elsewhere")).is_ok());
        assert_eq!(status_of(target.to_str().unwrap(), central.to_str().unwrap(), "commit"), LinkStatus::Broken);
        cleanup(&[&central, &target, &foreign]);
    }

    #[test]
    fn should_scan_batch_statuses() {
        let central = temp_dir("c6");
        let t_a = temp_dir("a6");
        let t_b = temp_dir("b6");
        let t_c = temp_dir("d6");
        make_skill(&central, "commit");
        link(t_a.to_str().unwrap(), central.to_str().unwrap(), "commit", true).unwrap();
        std::fs::create_dir_all(t_b.join("commit")).unwrap();
        // t_c 什么都不放
        assert_eq!(status_of(t_a.to_str().unwrap(), central.to_str().unwrap(), "commit"), LinkStatus::Linked);
        assert_eq!(status_of(t_b.to_str().unwrap(), central.to_str().unwrap(), "commit"), LinkStatus::Conflict);
        assert_eq!(status_of(t_c.to_str().unwrap(), central.to_str().unwrap(), "commit"), LinkStatus::NotLinked);
        // 检测只读：t_b 目录文件清单不变
        assert!(t_b.join("commit").is_dir());
        cleanup(&[&central, &t_a, &t_b, &t_c]);
    }

    #[test]
    fn should_create_junction_on_link() {
        let central = temp_dir("c7");
        let target = temp_dir("t7");
        make_skill(&central, "commit");
        link(target.to_str().unwrap(), central.to_str().unwrap(), "commit", true).unwrap();
        assert_eq!(status_of(target.to_str().unwrap(), central.to_str().unwrap(), "commit"), LinkStatus::Linked);
        // 幂等：重复 link 成功
        assert!(link(target.to_str().unwrap(), central.to_str().unwrap(), "commit", true).is_ok());
        cleanup(&[&central, &target]);
    }

    #[test]
    fn should_backup_then_overwrite_on_conflict() {
        let central = temp_dir("c8");
        let target = temp_dir("t8");
        make_skill(&central, "commit");
        std::fs::create_dir_all(target.join("commit")).unwrap();
        std::fs::write(target.join("commit").join("real.txt"), "keep me").unwrap();
        link(target.to_str().unwrap(), central.to_str().unwrap(), "commit", true).unwrap();
        assert_eq!(status_of(target.to_str().unwrap(), central.to_str().unwrap(), "commit"), LinkStatus::Linked);
        let bak = target.join("commit.bak");
        assert!(bak.is_dir(), "备份目录不存在");
        assert_eq!(std::fs::read_to_string(bak.join("real.txt")).unwrap(), "keep me");
        cleanup(&[&central, &target]);
    }

    #[test]
    fn should_append_timestamp_when_bak_exists() {
        let central = temp_dir("c9");
        let target = temp_dir("t9");
        make_skill(&central, "commit");
        std::fs::create_dir_all(target.join("commit")).unwrap();
        std::fs::create_dir_all(target.join("commit.bak")).unwrap();
        link(target.to_str().unwrap(), central.to_str().unwrap(), "commit", true).unwrap();
        assert_eq!(status_of(target.to_str().unwrap(), central.to_str().unwrap(), "commit"), LinkStatus::Linked);
        // 找到 commit.bak-<timestamp>
        let found = std::fs::read_dir(&target).unwrap()
            .filter_map(|e| e.ok())
            .find(|e| e.file_name().to_string_lossy().starts_with("commit.bak-"))
            .expect("时间戳备份目录不存在");
        assert!(found.path().is_dir());
        cleanup(&[&central, &target]);
    }

    #[test]
    fn should_delete_real_dir_when_overwrite_chosen() {
        let central = temp_dir("c10");
        let target = temp_dir("t10");
        make_skill(&central, "commit");
        std::fs::create_dir_all(target.join("commit")).unwrap();
        std::fs::write(target.join("commit").join("real.txt"), "gone").unwrap();
        link(target.to_str().unwrap(), central.to_str().unwrap(), "commit", false).unwrap();
        assert_eq!(status_of(target.to_str().unwrap(), central.to_str().unwrap(), "commit"), LinkStatus::Linked);
        assert!(!target.join("commit.bak").exists(), "overwrite 不应产生备份");
        cleanup(&[&central, &target]);
    }

    #[test]
    fn should_refuse_unlink_when_target_is_real_dir() {
        let target = temp_dir("t11");
        std::fs::create_dir_all(target.join("commit")).unwrap();
        std::fs::write(target.join("commit").join("real.txt"), "precious").unwrap();
        let before = std::fs::read_to_string(target.join("commit").join("real.txt")).unwrap();
        let result = unlink(target.to_str().unwrap(), "commit");
        assert!(result.is_err(), "unlink 真实目录必须报错");
        let after = std::fs::read_to_string(target.join("commit").join("real.txt")).unwrap();
        assert_eq!(before, after, "目录内容被改动");
        cleanup(&[&target]);
    }

    #[test]
    fn should_unlink_only_junction_not_content() {
        let central = temp_dir("c12");
        let target = temp_dir("t12");
        make_skill(&central, "commit");
        link(target.to_str().unwrap(), central.to_str().unwrap(), "commit", true).unwrap();
        unlink(target.to_str().unwrap(), "commit").unwrap();
        assert_eq!(status_of(target.to_str().unwrap(), central.to_str().unwrap(), "commit"), LinkStatus::NotLinked);
        // 中央仓库完好
        assert_eq!(std::fs::read_to_string(central.join("commit").join("extra.txt")).unwrap(), "payload");
        cleanup(&[&central, &target]);
    }

    #[test]
    fn should_link_all_skip_disabled_targets() {
        let central = temp_dir("c13");
        let on = temp_dir("on13");
        let off = temp_dir("off13");
        make_skill(&central, "commit");
        let targets = vec![
            ("On".to_string(), on.to_string_lossy().to_string(), true),
            ("Off".to_string(), off.to_string_lossy().to_string(), false),
        ];
        let results = link_all(central.to_str().unwrap(), "commit", &targets);
        assert_eq!(results.len(), 1, "停用 target 应被跳过");
        assert!(results[0].1.is_ok());
        assert_eq!(status_of(on.to_str().unwrap(), central.to_str().unwrap(), "commit"), LinkStatus::Linked);
        assert_eq!(status_of(off.to_str().unwrap(), central.to_str().unwrap(), "commit"), LinkStatus::NotLinked);
        cleanup(&[&central, &on, &off]);
    }

    #[test]
    fn should_propagate_mklink_failure() {
        let central = temp_dir("c14");
        let target = temp_dir("t14");
        // 中央仓库无该 skill → link 前置校验报错
        let result = link(target.to_str().unwrap(), central.to_str().unwrap(), "ghost", true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("中央仓库不存在该 skill"));
        cleanup(&[&central, &target]);
    }

    #[test]
    fn should_repair_broken_junction_on_link() {
        let central = temp_dir("c15");
        let target = temp_dir("t15");
        make_skill(&central, "commit");
        link(target.to_str().unwrap(), central.to_str().unwrap(), "commit", true).unwrap();
        std::fs::remove_dir_all(central.join("commit")).unwrap();
        make_skill(&central, "commit");
        // 重新 link 应修复悬空链接而不是静默成功
        link(target.to_str().unwrap(), central.to_str().unwrap(), "commit", true).unwrap();
        assert_eq!(status_of(target.to_str().unwrap(), central.to_str().unwrap(), "commit"), LinkStatus::Linked);
        cleanup(&[&central, &target]);
    }

    #[test]
    fn should_repair_foreign_junction_on_link() {
        let central = temp_dir("c16");
        let target = temp_dir("t16");
        let foreign = temp_dir("f16");
        make_skill(&central, "commit");
        std::fs::create_dir_all(foreign.join("elsewhere")).unwrap();
        assert!(create_junction(&target.join("commit"), &foreign.join("elsewhere")).is_ok());
        link(target.to_str().unwrap(), central.to_str().unwrap(), "commit", true).unwrap();
        assert_eq!(status_of(target.to_str().unwrap(), central.to_str().unwrap(), "commit"), LinkStatus::Linked);
        cleanup(&[&central, &target, &foreign]);
    }

    #[test]
    fn should_format_timestamp_as_yyyymmdd_hhmmss() {
        let ts = format_utc_timestamp();
        assert_eq!(ts.len(), 15, "格式应为 yyyyMMdd-HHmmss（15 字符）：{ts}");
        assert_eq!(ts.as_bytes()[8], b'-');
        assert!(ts.as_bytes()[..8].iter().all(|b| b.is_ascii_digit()));
        assert!(ts.as_bytes()[9..].iter().all(|b| b.is_ascii_digit()));
    }

    fn cleanup(dirs: &[&Path]) {
        for d in dirs {
            std::fs::remove_dir_all(d).ok();
        }
    }
}
