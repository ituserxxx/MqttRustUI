//! Topic 匹配与树构建。
//!
//! 订阅过滤器支持 MQTT 通配符：`+`（单层）、`#`（多层）。用于判断某条消息
//! 命中哪些订阅、以及在 UI 里把 topic 组织成树。
use std::collections::HashMap;

/// 判断 `topic` 是否匹配 `filter`（支持 + / #）。
pub fn matches(filter: &str, topic: &str) -> bool {
    let f = filter.split('/');
    let t = topic.split('/');
    let mut fi = f.peekable();
    let mut ti = t.peekable();

    // # 必须独占一层且只能是最后一段
    while let Some(fseg) = fi.next() {
        match fseg {
            "#" => {
                // # 匹配剩余所有层（含零层）
                return true;
            }
            "+" => {
                // 单层通配，要求 topic 还有下一层
                if ti.next().is_none() {
                    return false;
                }
            }
            _ => {
                let tseg = ti.next();
                match tseg {
                    Some(s) if s == fseg => continue,
                    _ => return false,
                }
            }
        }
    }
    // 过滤器用完，topic 也必须用完
    ti.next().is_none()
}

/// 一棵 topic 树节点。
#[derive(Debug, Default, Clone)]
pub struct TopicNode {
    pub children: HashMap<String, TopicNode>,
    pub full_path: String,
}

impl TopicNode {
    pub fn new() -> Self {
        TopicNode::default()
    }

    /// 把 `topic` 插入树（按 `/` 分层）。
    pub fn insert(&mut self, topic: &str) {
        let mut node = self;
        let mut path = String::new();
        for (i, seg) in topic.split('/').enumerate() {
            if i > 0 {
                path.push('/');
            }
            path.push_str(seg);
            node = node
                .children
                .entry(seg.to_string())
                .or_insert_with(|| TopicNode {
                    children: HashMap::new(),
                    full_path: path.clone(),
                });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard_match() {
        assert!(matches("sensors/#", "sensors/temp/room1"));
        assert!(matches("sensors/#", "sensors"));
        assert!(matches("sensors/+", "sensors/temp"));
        assert!(!matches("sensors/+", "sensors/temp/room1"));
        assert!(matches("a/b/c", "a/b/c"));
        assert!(!matches("a/b/c", "a/b/d"));
        assert!(!matches("a/+/c", "a/b/d"));
    }
}
