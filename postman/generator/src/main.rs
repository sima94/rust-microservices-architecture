use clap::Parser;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

// ── CLI ──────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "postman-gen",
    about = "Generate Postman collection from OpenAPI + overlay"
)]
struct Cli {
    /// Use cached OpenAPI specs instead of fetching from services
    #[arg(long)]
    offline: bool,

    /// Path to overlay.json
    #[arg(long)]
    overlay: Option<PathBuf>,

    /// Output path
    #[arg(short, long)]
    output: Option<PathBuf>,
}

// ── Overlay types ────────────────────────────────────────────────────

#[derive(Deserialize)]
struct Overlay {
    info: OverlayInfo,
    services: HashMap<String, ServiceConfig>,
    variables: HashMap<String, VariableValue>,
    scripts: HashMap<String, Vec<String>>,
    folders: Vec<FolderConfig>,
}

#[derive(Deserialize)]
struct OverlayInfo {
    name: String,
    description: String,
}

#[derive(Deserialize)]
struct ServiceConfig {
    openapi_url: String,
}

#[derive(Deserialize)]
struct VariableValue {
    value: String,
}

#[derive(Deserialize)]
struct FolderConfig {
    name: String,
    description: Option<String>,
    auth: Option<AuthConfig>,
    requests: Vec<RequestConfig>,
}

#[derive(Deserialize, Clone)]
struct AuthConfig {
    #[serde(rename = "type")]
    auth_type: String,
    token: String,
}

#[derive(Deserialize)]
struct RequestConfig {
    name: String,
    operation: Option<String>,
    service: Option<String>,
    url_base: Option<String>,
    path_params: Option<HashMap<String, String>>,
    query_override: Option<HashMap<String, String>>,
    custom: Option<bool>,
    method: Option<String>,
    url: Option<String>,
    content_type: Option<String>,
    body: Option<Value>,
    auth: Option<AuthConfig>,
    prerequest: Option<String>,
    save: Option<HashMap<String, String>>,
    expect_status: Option<u16>,
    accept_status: Option<Vec<u16>>,
}

// ── OpenAPI parsed info ──────────────────────────────────────────────

struct OperationInfo {
    method: String,
    path: String,
    content_type: Option<String>,
}

type OperationMap = HashMap<String, HashMap<String, OperationInfo>>;

// ── Main ─────────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();

    let postman_dir = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let overlay_path = cli
        .overlay
        .unwrap_or_else(|| postman_dir.join("overlay.json"));
    let output_path = cli
        .output
        .unwrap_or_else(|| postman_dir.join("Rust_Microservices.postman_collection.json"));
    let cache_dir = postman_dir.join("openapi");

    let overlay_text = fs::read_to_string(&overlay_path)
        .unwrap_or_else(|e| panic!("Cannot read {}: {e}", overlay_path.display()));
    let overlay: Overlay = serde_json::from_str(&overlay_text).expect("Invalid overlay.json");
    println!("Loaded overlay: {}", overlay_path.display());

    let specs = load_specs(&overlay.services, &cache_dir, cli.offline);
    let operations = parse_specs(&specs);

    for (svc, ops) in &operations {
        let names: Vec<&str> = ops.keys().map(|s| s.as_str()).collect();
        println!("  {svc}: {} operations ({})", ops.len(), names.join(", "));
    }

    let collection = generate_collection(&overlay, &operations);

    let out = serde_json::to_string_pretty(&collection).unwrap();
    fs::write(&output_path, &out)
        .unwrap_or_else(|e| panic!("Cannot write {}: {e}", output_path.display()));

    let total: usize = overlay.folders.iter().map(|f| f.requests.len()).sum();
    println!(
        "\nGenerated: {}\n  {total} requests in {} folders",
        output_path.display(),
        overlay.folders.len()
    );
}

// ── Fetch / cache OpenAPI specs ──────────────────────────────────────

fn load_specs(
    services: &HashMap<String, ServiceConfig>,
    cache_dir: &Path,
    offline: bool,
) -> HashMap<String, Value> {
    let mut specs = HashMap::new();

    for (name, config) in services {
        let cached = cache_dir.join(format!("{name}-service.json"));

        let spec: Value = if offline {
            println!("Loading cached {name} spec...");
            let text = fs::read_to_string(&cached).unwrap_or_else(|_| {
                panic!("No cached spec for {name}. Run without --offline first.")
            });
            serde_json::from_str(&text).expect("Invalid cached JSON")
        } else {
            println!("Fetching {name} OpenAPI from {}...", config.openapi_url);
            match reqwest::blocking::get(&config.openapi_url) {
                Ok(resp) if resp.status().is_success() => {
                    let spec: Value = resp.json().expect("Invalid OpenAPI JSON");
                    fs::create_dir_all(cache_dir).ok();
                    fs::write(&cached, serde_json::to_string_pretty(&spec).unwrap()).ok();
                    spec
                }
                _ => {
                    eprintln!("  Cannot fetch {name}, trying cache...");
                    let text = fs::read_to_string(&cached)
                        .unwrap_or_else(|_| panic!("No cached fallback for {name}"));
                    serde_json::from_str(&text).unwrap()
                }
            }
        };

        specs.insert(name.clone(), spec);
    }
    specs
}

// ── Parse OpenAPI into operation lookup ──────────────────────────────

fn parse_specs(specs: &HashMap<String, Value>) -> OperationMap {
    let mut all_ops = HashMap::new();

    for (service, spec) in specs {
        let mut ops = HashMap::new();
        let Some(paths) = spec.get("paths").and_then(|p| p.as_object()) else {
            all_ops.insert(service.clone(), ops);
            continue;
        };

        for (path, methods) in paths {
            let Some(methods) = methods.as_object() else {
                continue;
            };
            for (method, details) in methods {
                if !matches!(method.as_str(), "get" | "post" | "put" | "delete" | "patch") {
                    continue;
                }
                let Some(op_id) = details.get("operationId").and_then(|v| v.as_str()) else {
                    continue;
                };

                let content_type = details
                    .get("requestBody")
                    .and_then(|rb| rb.get("content"))
                    .and_then(|c| c.as_object())
                    .and_then(|c| {
                        if c.contains_key("application/json") {
                            Some("application/json")
                        } else if c.contains_key("application/x-www-form-urlencoded") {
                            Some("application/x-www-form-urlencoded")
                        } else {
                            None
                        }
                    })
                    .map(String::from);

                ops.insert(
                    op_id.to_string(),
                    OperationInfo {
                        method: method.to_uppercase(),
                        path: path.clone(),
                        content_type,
                    },
                );
            }
        }
        all_ops.insert(service.clone(), ops);
    }
    all_ops
}

// ── Generate Postman collection ──────────────────────────────────────

fn generate_collection(overlay: &Overlay, operations: &OperationMap) -> Value {
    let variables: Vec<Value> = overlay
        .variables
        .iter()
        .map(|(k, v)| json!({"key": k, "value": v.value, "type": "string"}))
        .collect();

    let folders: Vec<Value> = overlay
        .folders
        .iter()
        .map(|fc| build_folder(fc, operations, overlay))
        .collect();

    json!({
        "info": {
            "name": overlay.info.name,
            "description": overlay.info.description,
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
        },
        "variable": variables,
        "item": folders
    })
}

fn build_folder(folder: &FolderConfig, operations: &OperationMap, overlay: &Overlay) -> Value {
    let items: Vec<Value> = folder
        .requests
        .iter()
        .filter_map(|rc| build_item(rc, operations, overlay))
        .collect();

    let mut f = json!({"name": folder.name, "item": items});
    let obj = f.as_object_mut().unwrap();

    if let Some(desc) = &folder.description {
        obj.insert("description".into(), json!(desc));
    }
    if let Some(auth) = &folder.auth {
        obj.insert("auth".into(), build_auth(auth));
    }
    f
}

fn build_item(rc: &RequestConfig, operations: &OperationMap, overlay: &Overlay) -> Option<Value> {
    let request = if rc.custom.unwrap_or(false) {
        build_custom_request(rc)
    } else {
        build_openapi_request(rc, operations)?
    };

    let mut item = json!({"name": rc.name, "request": request});
    let mut events: Vec<Value> = Vec::new();

    if let Some(script_name) = &rc.prerequest {
        if let Some(lines) = overlay.scripts.get(script_name) {
            events.push(json!({
                "listen": "prerequest",
                "script": {"exec": lines}
            }));
        }
    }

    let test_lines = build_test_script(rc);
    if !test_lines.is_empty() {
        events.push(json!({
            "listen": "test",
            "script": {"exec": test_lines}
        }));
    }

    if !events.is_empty() {
        item.as_object_mut()
            .unwrap()
            .insert("event".into(), json!(events));
    }

    Some(item)
}

// ── Request builders ─────────────────────────────────────────────────

fn build_custom_request(rc: &RequestConfig) -> Value {
    let mut req = json!({
        "method": rc.method.as_deref().unwrap_or("GET"),
        "url": rc.url.as_deref().unwrap_or("")
    });
    let obj = req.as_object_mut().unwrap();

    if let Some(ct) = &rc.content_type {
        obj.insert(
            "header".into(),
            json!([{"key": "Content-Type", "value": ct}]),
        );
    }
    if let Some(body) = &rc.body {
        let ct = rc.content_type.as_deref().unwrap_or("application/json");
        obj.insert("body".into(), build_body(ct, body));
    }
    if let Some(auth) = &rc.auth {
        obj.insert("auth".into(), build_auth(auth));
    }
    req
}

fn build_openapi_request(rc: &RequestConfig, operations: &OperationMap) -> Option<Value> {
    let service = rc.service.as_ref()?;
    let op_id = rc.operation.as_ref()?;
    let op = operations.get(service)?.get(op_id);

    let Some(op) = op else {
        eprintln!("WARNING: operation '{}' not found in {service} spec", op_id);
        return None;
    };

    let url_base = rc.url_base.as_deref().unwrap_or("gateway_url");
    let mut path = op.path.clone();
    if let Some(params) = &rc.path_params {
        for (param, value) in params {
            path = path.replace(&format!("{{{param}}}"), value);
        }
    }

    let base = format!("{{{{{url_base}}}}}");

    let url = if let Some(query) = &rc.query_override {
        let query_items: Vec<Value> = query
            .iter()
            .map(|(k, v)| json!({"key": k, "value": v}))
            .collect();
        let qs = query
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");
        let path_parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();

        json!({
            "raw": format!("{base}{path}?{qs}"),
            "host": [base],
            "path": path_parts,
            "query": query_items
        })
    } else {
        json!(format!("{base}{path}"))
    };

    let mut req = json!({"method": op.method, "url": url});
    let obj = req.as_object_mut().unwrap();

    if let Some(body) = &rc.body {
        let ct = op.content_type.as_deref().unwrap_or("application/json");
        obj.insert(
            "header".into(),
            json!([{"key": "Content-Type", "value": ct}]),
        );
        obj.insert("body".into(), build_body(ct, body));
    }
    if let Some(auth) = &rc.auth {
        obj.insert("auth".into(), build_auth(auth));
    }

    Some(req)
}

fn build_body(content_type: &str, body: &Value) -> Value {
    if content_type == "application/x-www-form-urlencoded" {
        let urlencoded: Vec<Value> = body
            .as_object()
            .map(|o| {
                o.iter()
                    .map(|(k, v)| {
                        let val = match v {
                            Value::String(s) => s.clone(),
                            other => other.to_string(),
                        };
                        json!({"key": k, "value": val})
                    })
                    .collect()
            })
            .unwrap_or_default();
        json!({"mode": "urlencoded", "urlencoded": urlencoded})
    } else {
        json!({"mode": "raw", "raw": serde_json::to_string_pretty(body).unwrap_or_default()})
    }
}

fn build_auth(auth: &AuthConfig) -> Value {
    json!({
        "type": auth.auth_type,
        "bearer": [{"key": "token", "value": auth.token}]
    })
}

// ── Test script generation ───────────────────────────────────────────

fn build_test_script(rc: &RequestConfig) -> Vec<String> {
    let mut lines = Vec::new();
    let name = &rc.name;

    if let Some(accept) = &rc.accept_status {
        let primary = accept[0];
        lines.push(format!("if (pm.response.code === {primary}) {{"));

        if let Some(save) = &rc.save {
            lines.push("    var json = pm.response.json();".into());
            append_save_lines(&mut lines, save, "    ");
        }

        lines.push(format!("    pm.test('{name} - success', function () {{"));
        lines.push(format!("        pm.response.to.have.status({primary});"));
        lines.push("    });".into());

        for status in &accept[1..] {
            lines.push(format!("}} else if (pm.response.code === {status}) {{"));
            lines.push(format!(
                "    pm.test('{name} - already exists (OK)', function () {{"
            ));
            lines.push(format!("        pm.response.to.have.status({status});"));
            lines.push("    });".into());
        }
        lines.push("}".into());
    } else if let Some(save) = &rc.save {
        let status = rc.expect_status.unwrap_or(200);
        lines.push(format!("pm.test('{name} - success', function () {{"));
        lines.push(format!("    pm.response.to.have.status({status});"));
        lines.push("    var json = pm.response.json();".into());
        append_save_lines(&mut lines, save, "    ");
        lines.push("});".into());
    } else if let Some(status) = rc.expect_status {
        lines.push(format!("pm.test('{name}', function () {{"));
        lines.push(format!("    pm.response.to.have.status({status});"));
        lines.push("});".into());
    }

    lines
}

fn append_save_lines(lines: &mut Vec<String>, save: &HashMap<String, String>, indent: &str) {
    for (var_name, field_raw) in save {
        let (field, optional) = parse_optional_field(field_raw);
        if optional {
            lines.push(format!("{indent}if (json.{field}) {{"));
            lines.push(format!(
                "{indent}    pm.collectionVariables.set('{var_name}', json.{field});"
            ));
            lines.push(format!("{indent}}}"));
        } else {
            lines.push(format!(
                "{indent}pm.collectionVariables.set('{var_name}', json.{field});"
            ));
        }
    }
}

/// "field_name?" → ("field_name", true)
fn parse_optional_field(field: &str) -> (&str, bool) {
    field
        .strip_suffix('?')
        .map_or((field, false), |f| (f, true))
}
