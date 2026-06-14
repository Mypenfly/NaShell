use crate::error::NashellError;
use crate::plugin::manager::PluginHandle;
use crate::plugin::protocol::{send_message, PluginBroadcast, PluginMessage};

/// 向所有注册了广播的插件发送事件消息。
///
/// # 参数
/// - `event`: 事件名称（如 "shell_state_changed"、"cwd_changed"）
/// - `payload`: 事件载荷
/// - `plugins`: 所有插件的句柄切片
///
/// # 返回
/// 第一个发送失败的错误（如有），但不阻塞其他插件的发送。
/// 发送失败仅记录警告日志，不返回错误（单个插件失败不应影响其他插件）。
pub fn broadcast_event(
    event: &str,
    payload: &serde_json::Value,
    plugins: &mut [PluginHandle],
) -> Result<(), NashellError> {
    let msg = PluginMessage::Broadcast {
        sender: "nashell".to_string(),
        data: PluginBroadcast {
            event: event.to_string(),
            payload: payload.clone(),
        },
    };

    let mut last_error = None;

    for handle in plugins.iter_mut() {
        if !handle.meta.is_broadcast {
            continue;
        }

        if !handle.is_alive() {
            log::warn!(
                "跳过已退出插件 '{}' 的广播: {}",
                handle.name(),
                event
            );
            continue;
        }

        let stdin = match handle.child.stdin.as_mut() {
            Some(s) => s,
            None => {
                log::warn!("无法获取插件 '{}' 的 stdin", handle.name());
                continue;
            }
        };

        match send_message(stdin, &msg) {
            Ok(()) => {
                log::debug!("广播事件 '{}' 已发送给插件 '{}'", event, handle.name());
            }
            Err(e) => {
                log::warn!("广播事件 '{}' 发送给插件 '{}' 失败: {}", event, handle.name(), e);
                if last_error.is_none() {
                    last_error = Some(e);
                }
            }
        }
    }

    // Return the first error encountered, or Ok
    match last_error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_broadcast_event_empty_list() {
        let payload = serde_json::json!({"path": "/home/user"});
        let result = broadcast_event("cwd_changed", &payload, &mut []);
        assert!(result.is_ok());
    }
}
