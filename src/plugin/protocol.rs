use serde::{Deserialize, Serialize};

/// 插件消息的 type 字段
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginMsgType {
    /// 调用命令
    Call,
    /// 流式/分批响应
    Response,
    /// 结束通知
    Off,
    /// 广播消息
    Broadcast,
}

/// 主程序发给插件的 call 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCall {
    /// 命令名（小写）
    pub command: String,
    /// 子命令/模式
    pub mode: String,
    /// 命令级别
    pub level: String,
    /// 命令行参数
    pub params: Vec<String>,
    /// 多行长参数
    pub long_argument: Option<String>,
}

/// 插件发给主程序的 response 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResponse {
    /// 是否还有后续 response
    pub streaming: bool,
    /// 输出内容
    pub out_content: String,
    /// 输出提示符
    pub out_prompt: Option<String>,
    /// 输出提示符前景色（默认 "gray"）
    #[serde(default = "default_prompt_fg")]
    pub prompt_fg: String,
    /// 是否实时打印
    pub is_print: bool,
    /// 要求主程序代为执行的命令列表
    #[serde(default)]
    pub to_exec: Vec<String>,
    /// to_exec 的执行结果（由主程序填充后发回）
    pub exec_result: Option<Vec<String>>,
    /// 交互式输入请求
    #[serde(default)]
    pub get_input: Option<GetInput>,
    /// 用户输入结果（主程序发回给插件）
    #[serde(default)]
    pub user_input: Option<String>,
}

impl Default for PluginResponse {
    fn default() -> Self {
        PluginResponse {
            streaming: false,
            out_content: String::new(),
            out_prompt: None,
            prompt_fg: "gray".to_string(),
            is_print: false,
            to_exec: Vec::new(),
            exec_result: None,
            get_input: None,
            user_input: None,
        }
    }
}

/// 插件发给主程序的 off 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginOff {
    /// 关闭前要求执行的命令列表
    #[serde(default)]
    pub to_exec: Vec<String>,
    /// 最终输出内容
    pub out_content: String,
    /// 输出提示符
    pub out_prompt: Option<String>,
    /// 输出提示符前景色（默认 "gray"）
    #[serde(default = "default_prompt_fg")]
    pub prompt_fg: String,
    /// 是否打印
    pub is_print: bool,
}

impl Default for PluginOff {
    fn default() -> Self {
        PluginOff {
            to_exec: Vec::new(),
            out_content: String::new(),
            out_prompt: None,
            prompt_fg: "gray".to_string(),
            is_print: false,
        }
    }
}

/// 交互式输入请求，插件通过 response 的 get_input 字段向用户请求交互输入。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetInput {
    /// 输入提示前的内容（如警告信息）
    pub pre_content: Option<String>,
    /// pre_content 的前景色
    #[serde(default = "default_prompt_fg")]
    pub pre_fg: String,
    /// 输入提示符文本（如 "confirm (y/n) > "）
    pub input_prompt: String,
    /// 输入提示符前景色
    #[serde(default = "default_prompt_fg")]
    pub input_fg: String,
}

/// prompt_fg 等颜色字段的默认值
fn default_prompt_fg() -> String {
    "gray".to_string()
}

/// 主程序广播消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginBroadcast {
    /// 事件名称
    pub event: String,
    /// 事件载荷
    pub payload: serde_json::Value,
}

/// 插件通信中所有消息类型的枚举。
///
/// 用于统一的序列化/反序列化入口。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PluginMessage {
    /// 调用命令消息
    Call {
        /// 发送方标识
        sender: String,
        /// call 数据
        data: PluginCall,
    },
    /// 响应消息
    Response {
        /// 发送方标识
        sender: String,
        /// response 数据
        data: PluginResponse,
    },
    /// 结束通知消息
    Off {
        /// 发送方标识
        sender: String,
        /// off 数据
        data: PluginOff,
    },
    /// 广播消息
    Broadcast {
        /// 发送方标识
        sender: String,
        /// broadcast 数据
        data: PluginBroadcast,
    },
}

/// 向 writer 发送一条 NDJSON 消息。
///
/// 将消息序列化为单行 JSON 并写入。末尾追加换行符。
///
/// # 参数
/// - `writer`: 可写目标
/// - `msg`: 要发送的消息
///
/// # 错误
/// - IO 写入失败
/// - JSON 序列化失败
pub fn send_message(writer: &mut impl std::io::Write, msg: &PluginMessage) -> Result<(), crate::error::NashellError> {
    let json = serde_json::to_string(msg).map_err(|e| crate::error::NashellError::Plugin {
        plugin_name: "serialize".to_string(),
        detail: format!("序列化消息失败: {}", e),
    })?;
    writeln!(writer, "{}", json).map_err(|e| crate::error::NashellError::Io {
        path: None,
        source: e,
    })?;
    writer.flush().map_err(|e| crate::error::NashellError::Io {
        path: None,
        source: e,
    })?;
    Ok(())
}

/// 从 reader 读取一条 NDJSON 消息。
///
/// 读取一行完整 JSON，反序列化为 PluginMessage。
/// 空行会被跳过。
///
/// # 参数
/// - `reader`: 可读来源
///
/// # 返回
/// 解析后的消息
///
/// # 错误
/// - IO 读取失败
/// - JSON 反序列化失败
pub fn recv_message(reader: &mut impl std::io::BufRead) -> Result<PluginMessage, crate::error::NashellError> {
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = reader.read_line(&mut line).map_err(|e| crate::error::NashellError::Io {
            path: None,
            source: e,
        })?;
        if bytes == 0 {
            return Err(crate::error::NashellError::Plugin {
                plugin_name: "read".to_string(),
                detail: "插件连接已关闭（读取到 EOF）".to_string(),
            });
        }
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            let msg: PluginMessage = serde_json::from_str(trimmed).map_err(|e| crate::error::NashellError::Plugin {
                plugin_name: "deserialize".to_string(),
                detail: format!("反序列化消息失败: {} (原始: {})", e, trimmed),
            })?;
            return Ok(msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_plugin_call_serialization() {
        let call = PluginCall {
            command: "agent".to_string(),
            mode: "normal".to_string(),
            level: "system".to_string(),
            params: vec!["--model".to_string(), "deepseek-v4-pro".to_string()],
            long_argument: Some("some long text".to_string()),
        };
        let json = serde_json::to_string(&call).unwrap();
        let parsed: PluginCall = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.command, "agent");
        assert_eq!(parsed.mode, "normal");
        assert_eq!(parsed.level, "system");
        assert_eq!(parsed.params.len(), 2);
        assert_eq!(parsed.params[0], "--model");
        assert_eq!(parsed.params[1], "deepseek-v4-pro");
        assert_eq!(parsed.long_argument.as_deref(), Some("some long text"));
    }

    #[test]
    fn test_plugin_call_serialization_no_long_arg() {
        let call = PluginCall {
            command: "write".to_string(),
            mode: "normal".to_string(),
            level: "normal".to_string(),
            params: vec!["./test.txt".to_string()],
            long_argument: None,
        };
        let json = serde_json::to_string(&call).unwrap();
        let parsed: PluginCall = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.command, "write");
        assert_eq!(parsed.long_argument, None);
    }

    #[test]
    fn test_plugin_response_serialization() {
        let resp = PluginResponse {
            streaming: true,
            out_content: "Hello World".to_string(),
            out_prompt: Some("@agent #>>".to_string()),
            is_print: true,
            to_exec: vec!["ls -la".to_string()],
            exec_result: None,
            prompt_fg: "gray".to_string(),
            get_input: None,
            user_input: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: PluginResponse = serde_json::from_str(&json).unwrap();
        assert!(parsed.streaming);
        assert_eq!(parsed.out_content, "Hello World");
        assert_eq!(parsed.out_prompt.as_deref(), Some("@agent #>>"));
        assert!(parsed.is_print);
        assert_eq!(parsed.to_exec, vec!["ls -la"]);
        assert!(parsed.exec_result.is_none());
    }

    #[test]
    fn test_plugin_response_with_exec_result() {
        let resp = PluginResponse {
            streaming: false,
            out_content: String::new(),
            out_prompt: None,
            is_print: false,
            to_exec: vec!["echo hello".to_string()],
            exec_result: Some(vec!["hello".to_string()]),
            prompt_fg: "gray".to_string(),
            get_input: None,
            user_input: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: PluginResponse = serde_json::from_str(&json).unwrap();
        assert!(!parsed.streaming);
        assert_eq!(parsed.exec_result, Some(vec!["hello".to_string()]));
    }

    #[test]
    fn test_plugin_off_serialization() {
        let off = PluginOff {
            to_exec: vec![],
            out_content: "done".to_string(),
            out_prompt: Some("@agent #>>".to_string()),
            prompt_fg: "gray".to_string(),
            is_print: true,
        };
        let json = serde_json::to_string(&off).unwrap();
        let parsed: PluginOff = serde_json::from_str(&json).unwrap();
        assert!(parsed.to_exec.is_empty());
        assert_eq!(parsed.out_content, "done");
        assert!(parsed.is_print);
    }

    #[test]
    fn test_plugin_off_with_to_exec() {
        let off = PluginOff {
            to_exec: vec!["echo cleanup".to_string()],
            out_content: String::new(),
            out_prompt: None,
            prompt_fg: "gray".to_string(),
            is_print: false,
        };
        let json = serde_json::to_string(&off).unwrap();
        let parsed: PluginOff = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.to_exec, vec!["echo cleanup"]);
    }

    #[test]
    fn test_plugin_broadcast_serialization() {
        let broadcast = PluginBroadcast {
            event: "shell_state_changed".to_string(),
            payload: serde_json::json!({"name": "main", "path": "/home/user"}),
        };
        let json = serde_json::to_string(&broadcast).unwrap();
        let parsed: PluginBroadcast = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event, "shell_state_changed");
        assert_eq!(
            parsed.payload,
            serde_json::json!({"name": "main", "path": "/home/user"})
        );
    }

    #[test]
    fn test_plugin_message_call_serialization() {
        let msg = PluginMessage::Call {
            sender: "nashell".to_string(),
            data: PluginCall {
                command: "agent".to_string(),
                mode: "setting".to_string(),
                level: "system".to_string(),
                params: vec!["--model".to_string(), "deepseek".to_string()],
                long_argument: None,
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"call\""));
        assert!(json.contains("\"sender\":\"nashell\""));
        assert!(json.contains("\"command\":\"agent\""));
        assert!(json.contains("\"mode\":\"setting\""));
    }

    #[test]
    fn test_plugin_message_response_serialization() {
        let msg = PluginMessage::Response {
            sender: "agent".to_string(),
            data: PluginResponse {
                streaming: true,
                out_content: "Hello".to_string(),
                out_prompt: None,
                is_print: true,
                to_exec: vec![],
                exec_result: None,
                prompt_fg: "gray".to_string(),
                get_input: None,
                user_input: None,
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"response\""));
        assert!(json.contains("\"sender\":\"agent\""));
    }

    #[test]
    fn test_plugin_message_off_serialization() {
        let msg = PluginMessage::Off {
            sender: "agent".to_string(),
            data: PluginOff {
                to_exec: vec![],
                out_content: "done".to_string(),
                out_prompt: Some("@agent #>>".to_string()),
                prompt_fg: "gray".to_string(),
                is_print: true,
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"off\""));
    }

    #[test]
    fn test_send_and_recv_message() {
        let msg = PluginMessage::Call {
            sender: "nashell".to_string(),
            data: PluginCall {
                command: "test".to_string(),
                mode: "normal".to_string(),
                level: "normal".to_string(),
                params: vec!["--arg".to_string()],
                long_argument: Some("long text".to_string()),
            },
        };

        let mut buf = Vec::new();
        send_message(&mut buf, &msg).unwrap();

        let received = recv_message(&mut buf.as_slice()).unwrap();
        match received {
            PluginMessage::Call { sender, data } => {
                assert_eq!(sender, "nashell");
                assert_eq!(data.command, "test");
            }
            _ => panic!("expected Call message"),
        }
    }

    #[test]
    fn test_send_and_recv_response_message() {
        let msg = PluginMessage::Response {
            sender: "plugin".to_string(),
            data: PluginResponse {
                streaming: false,
                out_content: "result".to_string(),
                out_prompt: Some("@plugin #>>".to_string()),
                is_print: true,
                to_exec: vec!["echo done".to_string()],
                exec_result: Some(vec!["done".to_string()]),
                prompt_fg: "gray".to_string(),
                get_input: None,
                user_input: None,
            },
        };

        let mut buf = Vec::new();
        send_message(&mut buf, &msg).unwrap();

        let received = recv_message(&mut buf.as_slice()).unwrap();
        match received {
            PluginMessage::Response { sender, data } => {
                assert_eq!(sender, "plugin");
                assert!(!data.streaming);
                assert_eq!(data.out_content, "result");
            }
            _ => panic!("expected Response message"),
        }
    }

    #[test]
    fn test_recv_skips_empty_lines() {
        let msg = PluginMessage::Call {
            sender: "nashell".to_string(),
            data: PluginCall {
                command: "test".to_string(),
                mode: "normal".to_string(),
                level: "normal".to_string(),
                params: vec![],
                long_argument: None,
            },
        };

        let mut buf = Vec::new();
        // Write empty line first
        writeln!(buf, "").unwrap();
        writeln!(buf, "  ").unwrap();
        send_message(&mut buf, &msg).unwrap();

        let received = recv_message(&mut buf.as_slice()).unwrap();
        match received {
            PluginMessage::Call { data, .. } => {
                assert_eq!(data.command, "test");
            }
            _ => panic!("expected Call message"),
        }
    }

    #[test]
    fn test_plugin_response_empty_to_exec() {
        let resp = PluginResponse {
            streaming: false,
            out_content: "output".to_string(),
            out_prompt: None,
            is_print: false,
            to_exec: vec![],
            exec_result: None,
            prompt_fg: "gray".to_string(),
            get_input: None,
            user_input: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"to_exec\":[]"));
    }
}
