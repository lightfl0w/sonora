use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::preset::{VoicePreset, builtin_for};
use sonora_core::voice::{USER_ID_BASE, VoiceId, builtin};

#[derive(Clone, Debug)]
pub struct VoiceDef {
    pub id: VoiceId,
    pub name: String,
    pub preset: VoicePreset,
    pub builtin: bool,
}

#[derive(Clone, Debug)]
pub struct VoiceLibrary {
    voices: Vec<VoiceDef>,
    next_id: VoiceId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredVoice {
    pub name: String,
    pub preset: VoicePreset,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<VoiceId>,
}

impl VoiceLibrary {
    pub fn builtins() -> Self {
        let voices = builtin::ALL
            .iter()
            .map(|&id| VoiceDef {
                id,
                name: builtin_name(id).to_string(),
                preset: builtin_for(id),
                builtin: true,
            })
            .collect();
        Self {
            voices,
            next_id: USER_ID_BASE,
        }
    }
    pub fn load(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::builtins();
        };
        let Ok(stored) = serde_json::from_str::<Vec<StoredVoice>>(&text) else {
            return Self::builtins();
        };
        Self::with_user_voices(&stored)
    }
    pub fn load_default() -> Self {
        Self::load(&Self::default_path())
    }
    pub fn default_path() -> PathBuf {
        if let Ok(dir) = std::env::var("APPDATA") {
            PathBuf::from(dir).join("Sonora").join("voices.json")
        } else {
            PathBuf::from("sonora-voices.json")
        }
    }
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let stored = self.export_user();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&stored).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }
    pub fn save_default(&self) -> std::io::Result<()> {
        self.save(&Self::default_path())
    }
    pub fn export_user(&self) -> Vec<StoredVoice> {
        self.voices
            .iter()
            .filter(|v| !v.builtin)
            .map(|v| StoredVoice {
                name: v.name.clone(),
                preset: v.preset,
                id: Some(v.id),
            })
            .collect()
    }
    pub fn with_user_voices(user: &[StoredVoice]) -> Self {
        let mut lib = Self::builtins();
        for v in user {
            let mut preset = v.preset;
            preset.clamp_params();
            let id = match v
                .id
                .filter(|id| *id >= USER_ID_BASE && lib.get(*id).is_none())
            {
                Some(id) => id,
                None => lib.next_id,
            };
            lib.next_id = lib.next_id.max(id.saturating_add(1));
            lib.voices.push(VoiceDef {
                id,
                name: v.name.clone(),
                preset,
                builtin: false,
            });
        }
        lib
    }
    pub fn merge_user_voices(&mut self, user: &[StoredVoice]) -> usize {
        let mut added = 0;
        for v in user {
            let mut preset = v.preset;
            preset.clamp_params();
            let existing = self
                .voices
                .iter()
                .position(|d| !d.builtin && (Some(d.id) == v.id || d.name == v.name));
            if let Some(i) = existing {
                self.voices[i].preset = preset;
                self.voices[i].name = v.name.clone();
                continue;
            }
            let id = match v
                .id
                .filter(|id| *id >= USER_ID_BASE && self.get(*id).is_none())
            {
                Some(id) => id,
                None => self.next_id,
            };
            self.next_id = self.next_id.max(id.saturating_add(1));
            self.voices.push(VoiceDef {
                id,
                name: v.name.clone(),
                preset,
                builtin: false,
            });
            added += 1;
        }
        added
    }
    pub fn all(&self) -> &[VoiceDef] {
        &self.voices
    }
    pub fn get(&self, id: VoiceId) -> Option<&VoiceDef> {
        self.voices.iter().find(|v| v.id == id)
    }
    pub fn preset(&self, id: VoiceId) -> VoicePreset {
        self.get(id)
            .map(|v| v.preset)
            .unwrap_or_else(|| self.voices[0].preset)
    }
    pub fn name(&self, id: VoiceId) -> String {
        self.get(id)
            .map(|v| v.name.clone())
            .unwrap_or_else(|| "未知".to_string())
    }
    pub fn create_from(&mut self, template: VoiceId, name: String) -> VoiceId {
        let id = self.next_id;
        self.next_id += 1;
        let preset = self.preset(template);
        self.voices.push(VoiceDef {
            id,
            name,
            preset,
            builtin: false,
        });
        id
    }
    pub fn update_preset(&mut self, id: VoiceId, preset: VoicePreset) -> bool {
        let Some(v) = self.voices.iter_mut().find(|v| v.id == id) else {
            return false;
        };
        if v.builtin {
            return false;
        }
        let mut preset = preset;
        preset.clamp_params();
        v.preset = preset;
        true
    }
    pub fn rename(&mut self, id: VoiceId, name: String) -> bool {
        let Some(v) = self.voices.iter_mut().find(|v| v.id == id) else {
            return false;
        };
        if v.builtin {
            return false;
        }
        v.name = name;
        true
    }
    pub fn delete(&mut self, id: VoiceId) -> bool {
        let before = self.voices.len();
        self.voices.retain(|v| !(v.id == id && !v.builtin));
        self.voices.len() != before
    }
}

fn builtin_name(id: VoiceId) -> &'static str {
    match id {
        builtin::PIANO => "钢琴",
        builtin::STRINGS => "弦乐",
        builtin::HARP => "竖琴",
        builtin::AMBIENT => "环境音",
        builtin::BASS => "贝斯",
        builtin::DRUMS => "鼓组",
        _ => "未知",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preset;
    #[test]
    fn builtins_are_present_and_readonly() {
        let mut lib = VoiceLibrary::builtins();
        assert_eq!(lib.all().len(), builtin::COUNT);
        let modified = lib.update_preset(builtin::PIANO, preset::bass());
        assert!(!modified, "内置音色不可修改");
        let renamed = lib.rename(builtin::PIANO, "x".into());
        assert!(!renamed, "内置音色不可改名");
        assert!(!lib.delete(builtin::PIANO), "内置音色不可删除");
    }
    #[test]
    fn create_copy_and_delete() {
        let mut lib = VoiceLibrary::builtins();
        let id = lib.create_from(builtin::PIANO, "我的钢琴".into());
        assert!(id >= USER_ID_BASE);
        assert_eq!(lib.name(id), "我的钢琴");
        assert_eq!(lib.preset(id).count, lib.preset(builtin::PIANO).count);
        assert!(lib.update_preset(id, preset::strings()));
        assert_eq!(lib.preset(id).count, preset::strings().count);
        assert!(lib.delete(id));
        assert!(lib.get(id).is_none());
    }
    #[test]
    fn user_ids_do_not_collide() {
        let mut lib = VoiceLibrary::builtins();
        let a = lib.create_from(builtin::PIANO, "a".into());
        let b = lib.create_from(builtin::PIANO, "b".into());
        let c = lib.create_from(builtin::PIANO, "c".into());
        assert!(a < b && b < c);
        assert!(lib.all().iter().filter(|v| v.id == a).count() == 1);
    }
    #[test]
    fn stored_ids_survive_roundtrip() {
        let mut lib = VoiceLibrary::builtins();
        let a = lib.create_from(builtin::PIANO, "a".into());
        let b = lib.create_from(builtin::HARP, "b".into());
        let stored = lib.export_user();
        assert_eq!(stored[0].id, Some(a), "导出的用户音色应带上 id");
        let back = VoiceLibrary::with_user_voices(&stored);
        let ids: Vec<_> = back
            .all()
            .iter()
            .filter(|v| !v.builtin)
            .map(|v| v.id)
            .collect();
        assert_eq!(ids, vec![a, b], "载入后 id 必须保持不变（轨道引用它）");
        assert_eq!(back.name(a), "a");
    }
    #[test]
    fn merge_updates_by_id_and_appends_new() {
        let mut lib = VoiceLibrary::builtins();
        let id = lib.create_from(builtin::PIANO, "mine".into());
        let mut changed = lib.export_user();
        changed[0].preset = preset::strings();
        lib.merge_user_voices(&changed);
        assert_eq!(
            lib.all().iter().filter(|v| !v.builtin).count(),
            1,
            "同 id 应更新而不是重复添加"
        );
        assert_eq!(lib.preset(id).count, preset::strings().count);
        let extra = vec![StoredVoice {
            name: "另一个".into(),
            preset: preset::bass(),
            id: Some(USER_ID_BASE + 7),
        }];
        assert_eq!(lib.merge_user_voices(&extra), 1, "新 id 应追加");
        assert_eq!(lib.name(USER_ID_BASE + 7), "另一个");
        assert!(lib.get(id).is_some(), "原有音色不受影响");
    }
    #[test]
    fn legacy_stored_voices_without_id_still_load() {
        let legacy = StoredVoice {
            name: "old".into(),
            preset: preset::piano(),
            id: None,
        };
        let text = serde_json::to_string(&vec![legacy]).unwrap();
        assert!(!text.contains("\"id\""), "没有 id 时不应写进 json");
        let parsed: Vec<StoredVoice> = serde_json::from_str(&text).unwrap();
        assert!(parsed[0].id.is_none());
        let lib = VoiceLibrary::with_user_voices(&parsed);
        let custom: Vec<_> = lib.all().iter().filter(|v| !v.builtin).collect();
        assert_eq!(custom.len(), 1);
        assert_eq!(custom[0].id, USER_ID_BASE, "缺 id 时顺序分配");
    }
    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("sonora_lib_test");
        let path = dir.join("voices.json");
        let _ = std::fs::remove_file(&path);
        let mut lib = VoiceLibrary::builtins();
        let id = lib.create_from(builtin::STRINGS, "自定义弦乐".into());
        let mut custom = preset::strings();
        custom.attack = 0.8;
        lib.update_preset(id, custom);
        lib.save(&path).unwrap();
        let loaded = VoiceLibrary::load(&path);
        assert_eq!(loaded.all().len(), builtin::COUNT + 1);
        let restored = loaded.all().last().unwrap();
        assert_eq!(restored.name, "自定义弦乐");
        assert!((restored.preset.attack - 0.8).abs() < 1e-6);
        assert!(!restored.builtin);
        let _ = std::fs::remove_file(&path);
    }
    #[test]
    fn missing_file_falls_back_to_builtins() {
        let lib = VoiceLibrary::load(Path::new("/nonexistent/nope.json"));
        assert_eq!(lib.all().len(), builtin::COUNT);
    }
    #[test]
    fn corrupted_file_falls_back_to_builtins() {
        let dir = std::env::temp_dir().join("sonora_lib_test2");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.json");
        std::fs::write(&path, "{ not json").unwrap();
        let lib = VoiceLibrary::load(&path);
        assert_eq!(lib.all().len(), builtin::COUNT);
        let _ = std::fs::remove_file(&path);
    }
    #[test]
    fn loaded_preset_is_clamped() {
        let dir = std::env::temp_dir().join("sonora_lib_test3");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("wild.json");
        let mut wild = preset::piano();
        wild.attack = 999.0;
        wild.level = 99.0;
        let stored = serde_json::to_string(&vec![StoredVoice {
            name: "wild".into(),
            preset: wild,
            id: None,
        }])
        .unwrap();
        std::fs::write(&path, stored).unwrap();
        let lib = VoiceLibrary::load(&path);
        let v = lib.all().last().unwrap();
        assert!(v.preset.attack <= 4.0);
        assert!(v.preset.level <= 0.5);
        let _ = std::fs::remove_file(&path);
    }
}
