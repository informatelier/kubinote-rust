//! Il sottoinsieme di OpenAPI 3.0 che usa lo spec pubblicato da kubirag
//! (`contracts/golden/http/openapi.json`): match dei path con parametri, stati documentati e
//! validazione dei corpi JSON con `$ref`, `type`, `properties`, `required`, `items`,
//! `additionalProperties` e `nullable`. Basta per dire se una risposta rispetta il contratto
//! che la SPA si aspetta, senza una dipendenza da un validatore completo.

use serde_json::Value;

/// Lo spec e i suoi path compilati in segmenti.
pub struct Spec<'a> {
    spec: &'a Value,
    paths: Vec<(&'a str, Vec<Segment<'a>>)>,
}

enum Segment<'a> {
    Literal(&'a str),
    Parameter,
}

fn segments(path: &str) -> Vec<Segment<'_>> {
    path.trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| {
            if s.starts_with('{') && s.ends_with('}') {
                Segment::Parameter
            } else {
                Segment::Literal(s)
            }
        })
        .collect()
}

impl<'a> Spec<'a> {
    pub fn new(spec: &'a Value) -> Self {
        let paths = spec["paths"]
            .as_object()
            .expect("paths")
            .keys()
            .map(|p| (p.as_str(), segments(p)))
            .collect();
        Self { spec, paths }
    }

    /// Il path dello spec che corrisponde a `path`: a parità, quello con più segmenti letterali
    /// (`/notes/tags` prima di `/notes/{name}`).
    pub fn path_for(&self, path: &str) -> Option<&'a str> {
        let actual: Vec<&str> = path
            .trim_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();
        self.paths
            .iter()
            .filter(|(_, pattern)| {
                pattern.len() == actual.len()
                    && pattern.iter().zip(&actual).all(|(p, a)| match p {
                        Segment::Literal(l) => l == a,
                        Segment::Parameter => true,
                    })
            })
            .max_by_key(|(_, pattern)| {
                pattern
                    .iter()
                    .filter(|s| matches!(s, Segment::Literal(_)))
                    .count()
            })
            .map(|(p, _)| *p)
    }

    pub fn operation(&self, method: &str, path: &str) -> Option<&'a Value> {
        let template = self.path_for(path)?;
        self.spec["paths"][template]
            .get(method.to_ascii_lowercase())
            .filter(|op| op.is_object())
    }

    /// Le violazioni del contratto di una risposta: stato non documentato o corpo non conforme.
    pub fn check(
        &self,
        method: &str,
        path: &str,
        status: u16,
        body: Option<&Value>,
    ) -> Vec<String> {
        let Some(operation) = self.operation(method, path) else {
            return vec![format!("{method} {path}: operazione assente dall'OpenAPI")];
        };
        let responses = &operation["responses"];
        let Some(response) = responses
            .get(status.to_string())
            .or_else(|| responses.get("default"))
        else {
            return vec![format!("{method} {path}: stato {status} non documentato")];
        };
        let schema = response["content"]["application/json"]
            .get("schema")
            .filter(|_| body.is_some());
        match (schema, body) {
            (Some(schema), Some(body)) => {
                let mut errors = Vec::new();
                self.validate(schema, body, "$", &mut errors);
                errors
                    .into_iter()
                    .map(|e| format!("{method} {path} {status}: {e}"))
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    fn resolve(&self, schema: &'a Value) -> &'a Value {
        match schema.get("$ref").and_then(Value::as_str) {
            Some(reference) => {
                let name = reference.trim_start_matches("#/components/schemas/");
                self.resolve(&self.spec["components"]["schemas"][name])
            }
            None => schema,
        }
    }

    fn validate(&self, schema: &'a Value, value: &Value, at: &str, errors: &mut Vec<String>) {
        let schema = self.resolve(schema);
        if value.is_null() && schema["nullable"] == true {
            return;
        }
        if let Some(expected) = schema["type"].as_str() {
            let ok = match expected {
                "object" => value.is_object(),
                "array" => value.is_array(),
                "string" => value.is_string(),
                "boolean" => value.is_boolean(),
                "integer" => value.is_i64() || value.is_u64(),
                "number" => value.is_number(),
                _ => true,
            };
            if !ok {
                errors.push(format!("{at}: atteso {expected}, trovato {value}"));
                return;
            }
        }
        if let Some(object) = value.as_object() {
            for required in schema["required"].as_array().into_iter().flatten() {
                let name = required.as_str().unwrap_or("");
                if !object.contains_key(name) {
                    errors.push(format!("{at}: manca la proprietà obbligatoria {name}"));
                }
            }
            let properties = schema["properties"].as_object();
            for (name, item) in object {
                match properties.and_then(|p| p.get(name)) {
                    Some(property) => {
                        self.validate(property, item, &format!("{at}.{name}"), errors)
                    }
                    None => match &schema["additionalProperties"] {
                        Value::Bool(false) if properties.is_some() => {
                            errors.push(format!("{at}: proprietà non prevista {name}"));
                        }
                        additional @ Value::Object(_) => {
                            self.validate(additional, item, &format!("{at}.{name}"), errors);
                        }
                        _ => {}
                    },
                }
            }
        }
        if let (Some(items), Some(schema)) = (value.as_array(), schema.get("items")) {
            for (i, item) in items.iter().enumerate() {
                self.validate(schema, item, &format!("{at}[{i}]"), errors);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn spec() -> Value {
        json!({
            "paths": {
                "/notes/{name}": {"get": {"responses": {"200": {"content": {"application/json": {
                    "schema": {"$ref": "#/components/schemas/Note"}}}}}}},
                "/notes/tags": {"get": {"responses": {"200": {"content": {"application/json": {
                    "schema": {"type": "array", "items": {"type": "string"}}}}}}}},
            },
            "components": {"schemas": {"Note": {"type": "object", "required": ["name"],
                "properties": {"name": {"type": "string"}, "rev": {"type": "integer", "nullable": true}}}}}
        })
    }

    #[test]
    fn literal_paths_win_over_parameters() {
        let spec = spec();
        let spec = Spec::new(&spec);
        assert_eq!(spec.path_for("/notes/tags"), Some("/notes/tags"));
        assert_eq!(spec.path_for("/notes/a.md"), Some("/notes/{name}"));
        assert_eq!(spec.path_for("/other"), None);
    }

    #[test]
    fn validates_types_refs_and_statuses() {
        let spec = spec();
        let spec = Spec::new(&spec);
        assert!(spec
            .check(
                "GET",
                "/notes/a",
                200,
                Some(&json!({"name": "a", "rev": null}))
            )
            .is_empty());
        assert_eq!(
            spec.check("GET", "/notes/a", 200, Some(&json!({"rev": "x"})))
                .len(),
            2
        );
        assert_eq!(
            spec.check("GET", "/notes/tags", 200, Some(&json!(["a", 1])))
                .len(),
            1
        );
        assert_eq!(spec.check("GET", "/notes/a", 500, None).len(), 1);
    }
}
