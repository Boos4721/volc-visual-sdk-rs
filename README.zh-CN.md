# volc-visual-sdk

[English](README.md) | [中文](README.zh-CN.md)

火山引擎**智能视觉（CV）**服务的纯 Rust SDK，对标官方 Java/Python SDK。原生实现
火山引擎**签名 V4（HMAC-SHA256）**算法，TLS 由 **rustls** 提供——不依赖 OpenSSL。

## 特性

- 原生签名 V4，使用与官方 Python、Go SDK 完全一致的固定向量逐字节校验正确性。
- 智能视觉 API 通用的三类调用方式：
  - **同步** —— `cv_process`（`Action=CVProcess`）
  - **异步提交** —— `cv_submit_task`（`Action=CVSubmitTask`）
  - **异步查询** —— `cv_get_result`（`Action=CVGetResult`）
  - 以及三者共同封装的通用方法 `request(action, version, body)`。
- 强类型响应外壳（`ResponseMetadata` + `Result`），并用 `serde_json::Value`
  兜底各接口特有的返回结构。
- 可配置 region、host、STS 临时会话 token 与超时时间。
- 凭证支持显式传入，或从 `VOLC_ACCESSKEY` / `VOLC_SECRETKEY` 环境变量读取。

## 安装

```toml
[dependencies]
volc-visual-sdk = "0.1"
```

默认开启的 `blocking` feature 使用基于 rustls 的 `reqwest` 阻塞客户端。

## 快速上手

```rust
use volc_visual_sdk::VisualClient;
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 从环境变量读取 VOLC_ACCESSKEY / VOLC_SECRETKEY。
    let client = VisualClient::from_env()?
        .with_region("cn-north-1");

    let resp = client.cv_process(
        "CVProcess",
        "2022-08-31",
        json!({
            "req_key": "high_aes_general_v21_L",
            "prompt": "窗台上一只毛茸茸的猫"
        }),
    )?;

    if let Some(err) = resp.error() {
        eprintln!("接口错误: {err}");
    } else {
        println!("结果: {}", resp.result);
    }
    Ok(())
}
```

### 配置

```rust
use std::time::Duration;
use volc_visual_sdk::VisualClient;

let client = VisualClient::new("AK...", "SK...")
    .with_region("cn-north-1")
    .with_host("visual.volcengineapi.com")
    .with_security_token("STS2...")     // 可选的 STS 临时 token
    .with_timeout(Duration::from_secs(60));
```

## 三类接口

| 类型 | 方法 | Action | 说明 |
| --- | --- | --- | --- |
| 同步 | `cv_process` | `CVProcess` | 直接返回结果。 |
| 异步提交 | `cv_submit_task` | `CVSubmitTask` | 返回 `task_id`。 |
| 异步查询 | `cv_get_result` | `CVGetResult` | 轮询已提交任务的结果。 |

三者都转发到 `request(action, version, body)`，因此任何智能视觉接口——包括
同步转异步的 `CVSync2AsyncSubmitTask` / `CVSync2AsyncGetResult`——都能通过这个
通用方法调用。

## 签名 V4

每个请求都使用火山引擎签名 V4 进行签名：

1. **规范请求（Canonical Request）** = `method \n norm_uri \n norm_query \n canonical_headers \n signed_headers \n hex_sha256(body)`。
   参与签名的头部为 `content-type`、`host`、`x-content-sha256`、`x-date`，
   存在会话 token 时再加上 `x-security-token`——全部转小写并排序。
2. **待签字符串（StringToSign）** = `HMAC-SHA256 \n X-Date \n date/region/service/request \n hex_sha256(canonical_request)`。
3. **签名密钥（Signing Key）** = 四层链式 HMAC-SHA256：`date → region → service → "request"`。
4. **签名（Signature）** = `hex(HMAC-SHA256(signing_key, string_to_sign))`，连同
   credential scope 与 signed-header 列表写入 `Authorization` 头。

默认值：`service = cv`、`region = cn-north-1`、`host = visual.volcengineapi.com`，
`X-Date` 使用 ISO8601 basic 格式（`YYYYMMDDTHHMMSSZ`）。

签名核心 `sign::sign_with_date` 是确定性的，由 `src/sign.rs` 中的固定向量覆盖——
执行 `cargo test` 即可验证规范请求哈希、签名密钥与最终签名都与参考实现一致。

## 验证

这套固定向量不是自证循环：它们针对同一请求与两个官方 SDK 做了交叉比对，三个独立
实现（本 crate、Python SDK、Go SDK）对同一输入产出**字节级一致**的签名。

```bash
# 1) Rust 单元测试（规范请求哈希、签名密钥、最终签名）
cargo test

# 2) 与官方火山 Go SDK 交叉验证。
#    脚本会 clone volc-sdk-golang，向其 base 包注入一个测试以调用包内私有签名函数，
#    喂入与 src/sign.rs 相同的固定输入，断言 Go 签名器结果一致。
#    需要 Go 工具链与网络访问。
./scripts/verify_go_crossbuild.sh
```

| 验证源 | 方式 | 结果 |
| --- | --- | --- |
| 本 crate | `cargo test`（固定向量） | 一致 |
| 官方 Python SDK | `SignerV4.py` 跑相同输入 | 一致 |
| 官方 Go SDK | `scripts/verify_go_crossbuild.sh` | 一致 |

一个值得注意的正确性细节：SK 是以**原始 UTF-8 字节**参与第一层 HMAC 的——
**不做 base64 解码**，即使它看起来像 base64。两个官方 SDK 都如此，本 crate 也如此。

## 示例

```bash
# 同步文生图（通用2.1-文生图）
VOLC_ACCESSKEY=ak VOLC_SECRETKEY=sk cargo run --example text_to_image

# 异步提交 + 轮询
VOLC_ACCESSKEY=ak VOLC_SECRETKEY=sk cargo run --example async_task_poll
```

## 许可证

MIT
