use crate::error::{Result, ScriptError};
use crate::types::*;
use rquickjs::function::Rest;
use rquickjs::{Context, Ctx, Function, Object, Runtime, Value};
use serde_json::Value as JsonValue;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info, warn};

pub struct SandboxConfig {
    pub timeout_ms: u64,
    pub max_memory: usize,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 10000,
            max_memory: 16 * 1024 * 1024,
        }
    }
}

struct InterruptState {
    start_time: Instant,
    timeout_ms: u64,
    timed_out: AtomicBool,
}

impl InterruptState {
    fn new(timeout_ms: u64) -> Self {
        Self {
            start_time: Instant::now(),
            timeout_ms,
            timed_out: AtomicBool::new(false),
        }
    }

    fn check_timeout(&self) -> bool {
        let elapsed = self.start_time.elapsed().as_millis() as u64;
        if elapsed > self.timeout_ms {
            self.timed_out.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    fn is_timed_out(&self) -> bool {
        self.timed_out.load(Ordering::SeqCst)
    }
}

pub struct Sandbox {
    runtime: Runtime,
    context: Context,
    config: SandboxConfig,
    interrupt_state: Arc<InterruptState>,
}

impl Sandbox {
    pub fn new(config: SandboxConfig) -> Result<Self> {
        let runtime = Runtime::new().map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        runtime.set_memory_limit(config.max_memory);

        let interrupt_state = Arc::new(InterruptState::new(config.timeout_ms));
        let interrupt_state_clone = interrupt_state.clone();

        runtime.set_interrupt_handler(Some(Box::new(move || {
            interrupt_state_clone.check_timeout()
        })));

        let context =
            Context::full(&runtime).map_err(|e| ScriptError::QuickJsError(e.to_string()))?;

        debug!(
            target: "bifrost::script",
            timeout_ms = config.timeout_ms,
            max_memory = config.max_memory,
            "Sandbox created with timeout and memory limits"
        );

        Ok(Self {
            runtime,
            context,
            config,
            interrupt_state,
        })
    }

    fn reset_interrupt_state(&mut self) {
        let new_state = Arc::new(InterruptState::new(self.config.timeout_ms));
        let state_clone = new_state.clone();
        self.runtime
            .set_interrupt_handler(Some(Box::new(move || state_clone.check_timeout())));
        self.interrupt_state = new_state;
    }

    fn check_and_return_timeout_error<T>(&self) -> Option<Result<T>> {
        if self.interrupt_state.is_timed_out() {
            Some(Err(ScriptError::Timeout(self.config.timeout_ms)))
        } else {
            None
        }
    }

    pub fn execute_request_script(
        &mut self,
        script: &str,
        request: &RequestData,
        ctx: &ScriptContext,
    ) -> Result<(RequestModifications, Vec<ScriptLogEntry>)> {
        self.reset_interrupt_state();

        let logs = Rc::new(RefCell::new(Vec::new()));
        let modifications = Rc::new(RefCell::new(RequestModifications::default()));
        let request_headers = Rc::new(RefCell::new(request.headers.clone()));
        let request_body = Rc::new(RefCell::new(request.body.clone()));
        let request_method = Rc::new(RefCell::new(request.method.clone()));

        let eval_result = self.context.with(|js_ctx| {
            self.setup_log_object(
                &js_ctx,
                logs.clone(),
                ctx.script_name.clone(),
                ctx.request_id.clone(),
            )?;
            self.setup_ctx_object(&js_ctx, ctx)?;
            self.setup_request_object(
                &js_ctx,
                request,
                request_headers.clone(),
                request_body.clone(),
                request_method.clone(),
            )?;
            self.remove_dangerous_globals(&js_ctx)?;

            if let Err(e) = js_ctx.eval::<(), _>(script) {
                let exc = js_ctx.catch();
                let err_detail = if exc.is_exception() || exc.is_error() {
                    if let Some(exc_obj) = exc.as_object() {
                        let msg = exc_obj.get::<_, String>("message").ok().unwrap_or_default();
                        let stack = exc_obj.get::<_, String>("stack").ok().unwrap_or_default();
                        if !msg.is_empty() {
                            if !stack.is_empty() {
                                format!("{}\n{}", msg, stack)
                            } else {
                                msg
                            }
                        } else {
                            format!("{:?}", exc)
                        }
                    } else if let Some(exc_str) = exc.clone().into_string() {
                        exc_str.to_string().unwrap_or_else(|_| format!("{:?}", exc))
                    } else {
                        format!("{:?}", exc)
                    }
                } else {
                    format!("{:?}", exc)
                };
                return Err(ScriptError::ExecutionFailed(format!(
                    "{}: {}",
                    e, err_detail
                )));
            }

            let request_snapshot: String = js_ctx
                .eval("JSON.stringify(request)")
                .map_err(|e| ScriptError::ExecutionFailed(e.to_string()))?;
            if let Ok(snapshot) = serde_json::from_str::<JsonValue>(&request_snapshot) {
                if let Some(obj) = snapshot.as_object() {
                    if let Some(method) = obj.get("method").and_then(|v| v.as_str()) {
                        *request_method.borrow_mut() = method.to_string();
                    }
                    match obj.get("body") {
                        Some(JsonValue::Null) | None => {
                            *request_body.borrow_mut() = None;
                        }
                        Some(value) => {
                            *request_body.borrow_mut() =
                                Some(value.to_string().trim_matches('"').to_string());
                        }
                    }
                    if let Some(headers) = obj.get("headers").and_then(|v| v.as_object()) {
                        let mut new_headers = HashMap::new();
                        for (k, v) in headers {
                            let value = v
                                .as_str()
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| v.to_string());
                            new_headers.insert(k.to_string(), value);
                        }
                        *request_headers.borrow_mut() = new_headers;
                    }
                }
            }

            Ok::<(), ScriptError>(())
        });

        if let Some(timeout_err) = self.check_and_return_timeout_error() {
            warn!(
                target: "bifrost::script",
                script_name = %ctx.script_name,
                timeout_ms = self.config.timeout_ms,
                "Script execution timed out"
            );
            return timeout_err;
        }

        eval_result?;

        let mut mods = modifications.borrow_mut();
        let headers = request_headers.borrow();
        let body = request_body.borrow();
        let method = request_method.borrow();

        if *headers != request.headers {
            mods.headers = Some(headers.clone());
        }
        if *body != request.body {
            mods.body = body.clone();
        }
        if *method != request.method {
            mods.method = Some(method.clone());
        }

        let logs_result = logs.borrow().clone();
        let mods_result = mods.clone();

        Ok((mods_result, logs_result))
    }

    pub fn execute_response_script(
        &mut self,
        script: &str,
        response: &ResponseData,
        ctx: &ScriptContext,
    ) -> Result<(ResponseModifications, Vec<ScriptLogEntry>)> {
        self.reset_interrupt_state();

        let logs = Rc::new(RefCell::new(Vec::new()));
        let modifications = Rc::new(RefCell::new(ResponseModifications::default()));
        let response_headers = Rc::new(RefCell::new(response.headers.clone()));
        let response_body = Rc::new(RefCell::new(response.body.clone()));
        let response_status = Rc::new(RefCell::new(response.status));
        let response_status_text = Rc::new(RefCell::new(response.status_text.clone()));

        let eval_result = self.context.with(|js_ctx| {
            self.setup_log_object(
                &js_ctx,
                logs.clone(),
                ctx.script_name.clone(),
                ctx.request_id.clone(),
            )?;
            self.setup_ctx_object(&js_ctx, ctx)?;
            self.setup_response_object(
                &js_ctx,
                response,
                response_headers.clone(),
                response_body.clone(),
                response_status.clone(),
                response_status_text.clone(),
            )?;
            self.remove_dangerous_globals(&js_ctx)?;

            if let Err(e) = js_ctx.eval::<(), _>(script) {
                let exc = js_ctx.catch();
                let err_detail = if exc.is_exception() || exc.is_error() {
                    if let Some(exc_obj) = exc.as_object() {
                        let msg = exc_obj.get::<_, String>("message").ok().unwrap_or_default();
                        let stack = exc_obj.get::<_, String>("stack").ok().unwrap_or_default();
                        if !msg.is_empty() {
                            if !stack.is_empty() {
                                format!("{}\n{}", msg, stack)
                            } else {
                                msg
                            }
                        } else {
                            format!("{:?}", exc)
                        }
                    } else if let Some(exc_str) = exc.clone().into_string() {
                        exc_str.to_string().unwrap_or_else(|_| format!("{:?}", exc))
                    } else {
                        format!("{:?}", exc)
                    }
                } else {
                    format!("{:?}", exc)
                };
                return Err(ScriptError::ExecutionFailed(format!(
                    "{}: {}",
                    e, err_detail
                )));
            }

            let response_snapshot: String = js_ctx
                .eval("JSON.stringify(response)")
                .map_err(|e| ScriptError::ExecutionFailed(e.to_string()))?;
            if let Ok(snapshot) = serde_json::from_str::<JsonValue>(&response_snapshot) {
                if let Some(obj) = snapshot.as_object() {
                    if let Some(status) = obj.get("status").and_then(|v| v.as_u64()) {
                        *response_status.borrow_mut() = status as u16;
                    }
                    if let Some(status_text) = obj.get("statusText").and_then(|v| v.as_str()) {
                        *response_status_text.borrow_mut() = status_text.to_string();
                    }
                    match obj.get("body") {
                        Some(JsonValue::Null) | None => {
                            *response_body.borrow_mut() = None;
                        }
                        Some(value) => {
                            *response_body.borrow_mut() =
                                Some(value.to_string().trim_matches('"').to_string());
                        }
                    }
                    if let Some(headers) = obj.get("headers").and_then(|v| v.as_object()) {
                        let mut new_headers = HashMap::new();
                        for (k, v) in headers {
                            let value = v
                                .as_str()
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| v.to_string());
                            new_headers.insert(k.to_string(), value);
                        }
                        *response_headers.borrow_mut() = new_headers;
                    }
                }
            }

            Ok::<(), ScriptError>(())
        });

        if let Some(timeout_err) = self.check_and_return_timeout_error() {
            warn!(
                target: "bifrost::script",
                script_name = %ctx.script_name,
                timeout_ms = self.config.timeout_ms,
                "Script execution timed out"
            );
            return timeout_err;
        }

        eval_result?;

        let mut mods = modifications.borrow_mut();
        let headers = response_headers.borrow();
        let body = response_body.borrow();
        let status = *response_status.borrow();
        let status_text = response_status_text.borrow();

        if *headers != response.headers {
            mods.headers = Some(headers.clone());
        }
        if *body != response.body {
            mods.body = body.clone();
        }
        if status != response.status {
            mods.status = Some(status);
        }
        if *status_text != response.status_text {
            mods.status_text = Some(status_text.clone());
        }

        let logs_result = logs.borrow().clone();
        let mods_result = mods.clone();

        Ok((mods_result, logs_result))
    }

    fn setup_log_object(
        &self,
        js_ctx: &Ctx,
        logs: Rc<RefCell<Vec<ScriptLogEntry>>>,
        script_name: String,
        request_id: String,
    ) -> Result<()> {
        let log =
            Object::new(js_ctx.clone()).map_err(|e| ScriptError::QuickJsError(e.to_string()))?;

        for level in ["log", "debug", "info", "warn", "error"] {
            let logs_clone = logs.clone();
            let script_name_clone = script_name.clone();
            let request_id_clone = request_id.clone();
            let level_enum = match level {
                "log" => ScriptLogLevel::Info,
                "debug" => ScriptLogLevel::Debug,
                "info" => ScriptLogLevel::Info,
                "warn" => ScriptLogLevel::Warn,
                "error" => ScriptLogLevel::Error,
                _ => unreachable!(),
            };

            let func = Function::new(js_ctx.clone(), move |args: Rest<Value>| {
                let message = args
                    .0
                    .iter()
                    .map(|v| {
                        if let Some(s) = v.as_string() {
                            if let Ok(s) = s.to_string() {
                                return s;
                            }
                        }
                        format!("{:?}", v)
                    })
                    .collect::<Vec<_>>()
                    .join(" ");

                match level_enum {
                    ScriptLogLevel::Debug => {
                        debug!(
                            target: "bifrost::script",
                            script = %script_name_clone,
                            request_id = %request_id_clone,
                            "{}",
                            message
                        );
                    }
                    ScriptLogLevel::Info => {
                        info!(
                            target: "bifrost::script",
                            script = %script_name_clone,
                            request_id = %request_id_clone,
                            "{}",
                            message
                        );
                    }
                    ScriptLogLevel::Warn => {
                        warn!(
                            target: "bifrost::script",
                            script = %script_name_clone,
                            request_id = %request_id_clone,
                            "{}",
                            message
                        );
                    }
                    ScriptLogLevel::Error => {
                        error!(
                            target: "bifrost::script",
                            script = %script_name_clone,
                            request_id = %request_id_clone,
                            "{}",
                            message
                        );
                    }
                }

                logs_clone.borrow_mut().push(ScriptLogEntry {
                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                    level: level_enum,
                    message,
                    args: None,
                });
            })
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;

            log.set(level, func)
                .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        }

        let globals = js_ctx.globals();
        globals
            .set("log", log.clone())
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        globals
            .set("console", log)
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;

        Ok(())
    }

    fn setup_ctx_object(&self, js_ctx: &Ctx, script_ctx: &ScriptContext) -> Result<()> {
        let ctx_obj =
            Object::new(js_ctx.clone()).map_err(|e| ScriptError::QuickJsError(e.to_string()))?;

        ctx_obj
            .set("requestId", script_ctx.request_id.clone())
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        ctx_obj
            .set("scriptName", script_ctx.script_name.clone())
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        ctx_obj
            .set("scriptType", script_ctx.script_type.to_string())
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;

        let values_obj =
            Object::new(js_ctx.clone()).map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        for (k, v) in &script_ctx.values {
            values_obj
                .set(k.as_str(), v.clone())
                .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        }
        ctx_obj
            .set("values", values_obj)
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;

        let rules_array = rquickjs::Array::new(js_ctx.clone())
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        for (i, rule) in script_ctx.matched_rules.iter().enumerate() {
            let rule_obj = Object::new(js_ctx.clone())
                .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
            rule_obj
                .set("pattern", rule.pattern.clone())
                .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
            rule_obj
                .set("protocol", rule.protocol.clone())
                .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
            rule_obj
                .set("value", rule.value.clone())
                .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
            rules_array
                .set(i, rule_obj)
                .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        }
        ctx_obj
            .set("matchedRules", rules_array)
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;

        let globals = js_ctx.globals();
        globals
            .set("ctx", ctx_obj)
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;

        Ok(())
    }

    fn setup_request_object(
        &self,
        js_ctx: &Ctx,
        request: &RequestData,
        _headers: Rc<RefCell<HashMap<String, String>>>,
        _body: Rc<RefCell<Option<String>>>,
        _method: Rc<RefCell<String>>,
    ) -> Result<()> {
        let req =
            Object::new(js_ctx.clone()).map_err(|e| ScriptError::QuickJsError(e.to_string()))?;

        req.set("url", request.url.clone())
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        req.set("host", request.host.clone())
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        req.set("path", request.path.clone())
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        req.set("protocol", request.protocol.clone())
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        req.set("clientIp", request.client_ip.clone())
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        req.set("clientApp", request.client_app.clone())
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        req.set("method", request.method.clone())
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;

        let headers_obj =
            Object::new(js_ctx.clone()).map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        for (k, v) in &request.headers {
            headers_obj
                .set(k.as_str(), v.clone())
                .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        }
        req.set("headers", headers_obj)
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;

        if let Some(b) = &request.body {
            req.set("body", b.clone())
                .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        } else {
            req.set("body", Value::new_null(js_ctx.clone()))
                .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        }

        let globals = js_ctx.globals();
        globals
            .set("request", req)
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;

        Ok(())
    }

    fn setup_response_object(
        &self,
        js_ctx: &Ctx,
        response: &ResponseData,
        _headers: Rc<RefCell<HashMap<String, String>>>,
        _body: Rc<RefCell<Option<String>>>,
        _status: Rc<RefCell<u16>>,
        _status_text: Rc<RefCell<String>>,
    ) -> Result<()> {
        let res =
            Object::new(js_ctx.clone()).map_err(|e| ScriptError::QuickJsError(e.to_string()))?;

        res.set("status", response.status)
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        res.set("statusText", response.status_text.clone())
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;

        let headers_obj =
            Object::new(js_ctx.clone()).map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        for (k, v) in &response.headers {
            headers_obj
                .set(k.as_str(), v.clone())
                .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        }
        res.set("headers", headers_obj)
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;

        if let Some(b) = &response.body {
            res.set("body", b.clone())
                .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        } else {
            res.set("body", Value::new_null(js_ctx.clone()))
                .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        }

        let req_obj =
            Object::new(js_ctx.clone()).map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        req_obj
            .set("url", response.request.url.clone())
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        req_obj
            .set("method", response.request.method.clone())
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        req_obj
            .set("host", response.request.host.clone())
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        req_obj
            .set("path", response.request.path.clone())
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;

        let req_headers_obj =
            Object::new(js_ctx.clone()).map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        for (k, v) in &response.request.headers {
            req_headers_obj
                .set(k.as_str(), v.clone())
                .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;
        }
        req_obj
            .set("headers", req_headers_obj)
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;

        res.set("request", req_obj)
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;

        let globals = js_ctx.globals();
        globals
            .set("response", res)
            .map_err(|e| ScriptError::QuickJsError(e.to_string()))?;

        Ok(())
    }

    fn remove_dangerous_globals(&self, js_ctx: &Ctx) -> Result<()> {
        let globals = js_ctx.globals();

        let dangerous = ["eval", "Function"];
        for name in dangerous {
            let _ = globals.remove(name);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_creation() {
        let sandbox = Sandbox::new(SandboxConfig::default());
        assert!(sandbox.is_ok());
    }

    #[test]
    fn test_simple_script() {
        let mut sandbox = Sandbox::new(SandboxConfig::default()).unwrap();
        let request = RequestData {
            url: "https://example.com/api".to_string(),
            method: "GET".to_string(),
            host: "example.com".to_string(),
            path: "/api".to_string(),
            protocol: "https".to_string(),
            client_ip: "127.0.0.1".to_string(),
            client_app: Some("test".to_string()),
            headers: HashMap::new(),
            body: None,
        };

        let ctx = ScriptContext {
            request_id: "test-123".to_string(),
            script_name: "test".to_string(),
            script_type: ScriptType::Request,
            values: HashMap::new(),
            matched_rules: vec![],
        };

        let script = r#"
            log.info("Processing request: " + request.url);
        "#;

        let result = sandbox.execute_request_script(script, &request, &ctx);
        assert!(result.is_ok());

        let (_, logs) = result.unwrap();
        assert_eq!(logs.len(), 1);
        assert!(logs[0].message.contains("example.com"));
    }

    #[test]
    fn test_ctx_values_access() {
        let mut sandbox = Sandbox::new(SandboxConfig::default()).unwrap();
        let request = RequestData {
            url: "https://example.com/api".to_string(),
            method: "GET".to_string(),
            host: "example.com".to_string(),
            path: "/api".to_string(),
            protocol: "https".to_string(),
            client_ip: "127.0.0.1".to_string(),
            client_app: None,
            headers: HashMap::new(),
            body: None,
        };

        let mut values = HashMap::new();
        values.insert("API_TOKEN".to_string(), "secret-token".to_string());

        let ctx = ScriptContext {
            request_id: "test-123".to_string(),
            script_name: "test".to_string(),
            script_type: ScriptType::Request,
            values,
            matched_rules: vec![],
        };

        let script = r#"
            var token = ctx.values.API_TOKEN;
            log.info("Token: " + token);
        "#;

        let result = sandbox.execute_request_script(script, &request, &ctx);
        assert!(result.is_ok());

        let (_, logs) = result.unwrap();
        assert_eq!(logs.len(), 1);
        assert!(logs[0].message.contains("secret-token"));
    }

    #[test]
    fn test_infinite_loop_timeout() {
        let mut sandbox = Sandbox::new(SandboxConfig {
            timeout_ms: 100,
            max_memory: 16 * 1024 * 1024,
        })
        .unwrap();

        let request = RequestData {
            url: "https://example.com/api".to_string(),
            method: "GET".to_string(),
            host: "example.com".to_string(),
            path: "/api".to_string(),
            protocol: "https".to_string(),
            client_ip: "127.0.0.1".to_string(),
            client_app: None,
            headers: HashMap::new(),
            body: None,
        };

        let ctx = ScriptContext {
            request_id: "test-timeout".to_string(),
            script_name: "timeout-test".to_string(),
            script_type: ScriptType::Request,
            values: HashMap::new(),
            matched_rules: vec![],
        };

        let script = r#"
            while(true) {}
        "#;

        let start = std::time::Instant::now();
        let result = sandbox.execute_request_script(script, &request, &ctx);
        let elapsed = start.elapsed();

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ScriptError::Timeout(_)));
        assert!(
            elapsed.as_millis() < 500,
            "Timeout should trigger within 500ms, took {:?}",
            elapsed
        );
    }

    #[test]
    fn test_cpu_intensive_timeout() {
        let mut sandbox = Sandbox::new(SandboxConfig {
            timeout_ms: 100,
            max_memory: 16 * 1024 * 1024,
        })
        .unwrap();

        let request = RequestData {
            url: "https://example.com/api".to_string(),
            method: "GET".to_string(),
            host: "example.com".to_string(),
            path: "/api".to_string(),
            protocol: "https".to_string(),
            client_ip: "127.0.0.1".to_string(),
            client_app: None,
            headers: HashMap::new(),
            body: None,
        };

        let ctx = ScriptContext {
            request_id: "test-cpu".to_string(),
            script_name: "cpu-test".to_string(),
            script_type: ScriptType::Request,
            values: HashMap::new(),
            matched_rules: vec![],
        };

        let script = r#"
            var sum = 0;
            for (var i = 0; i < 1000000000; i++) {
                sum += i;
            }
        "#;

        let start = std::time::Instant::now();
        let result = sandbox.execute_request_script(script, &request, &ctx);
        let elapsed = start.elapsed();

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ScriptError::Timeout(_)));
        assert!(
            elapsed.as_millis() < 500,
            "Timeout should trigger within 500ms, took {:?}",
            elapsed
        );
    }
}
