use std::collections::HashMap;

use hyper::{body::to_bytes, Body, Request, Response};
use rlua::UserData;

#[derive(Clone, Debug)]
pub struct ProxyRequest {
    uri: String,
    method: String,
    headers: HashMap<String, String>,
    body: String,
}

impl ProxyRequest {
    pub async fn from(request: Request<Body>) -> Result<Self, hyper::Error> {
        let (parts, body) = request.into_parts();
        let headers: HashMap<String, String> = parts
            .headers
            .iter()
            .map(|(key, val)| {
                (
                    key.to_string(),
                    String::from_utf8_lossy(val.as_bytes()).to_string(),
                )
            })
            .collect();

        let body = &to_bytes(body).await?;
        let body = String::from_utf8_lossy(body);

        Ok(ProxyRequest {
            uri: parts.uri.to_string(),
            method: parts.method.to_string(),
            body: body.to_string(),
            headers,
        })
    }

    pub fn into_request(self) -> anyhow::Result<Request<Body>> {
        let mut request = Request::builder()
            .method(self.method.as_str())
            .uri(self.uri.as_str());

        for (key, value) in self.headers {
            request = request.header(key.as_str(), value.as_str());
        }

        Ok(request.body(Body::from(self.body))?)
    }

    pub fn _new(uri: &str, method: &str, body: &str) -> Self {
        Self {
            uri: uri.to_string(),
            method: method.to_string(),
            headers: Default::default(),
            body: body.to_string(),
        }
    }

    pub fn _with_header(&mut self, key: &str, val: &str) {
        let _ = self.headers.insert(key.to_string(), val.to_string());
    }
}

impl UserData for ProxyRequest {
    fn add_methods<'lua, T: rlua::UserDataMethods<'lua, Self>>(methods: &mut T) {
        methods.add_method("uri", |_, req, ()| Ok(req.uri.to_string()));
        methods.add_method("method", |_, req, ()| Ok(req.method.to_string()));
        methods.add_method("body", |_, req, ()| Ok(req.body.to_string()));
        methods.add_method("headers", |_, req, ()| Ok(req.headers.clone()));

        methods.add_method_mut("set_uri", |_, req, (uri,)| {
            req.uri = uri;
            Ok(())
        });
        methods.add_method_mut("set_method", |_, req, (method,)| {
            req.method = method;
            Ok(())
        });
        methods.add_method_mut("set_body", |_, req, (body,): (String,)| {
            req.headers
                .insert("content-length".to_string(), body.len().to_string());
            req.body = body;
            Ok(())
        });
        methods.add_method_mut("set_header", |_, req, (key, value)| {
            let _ = req.headers.insert(key, value);
            Ok(())
        });
    }
}

#[derive(Clone, Debug)]
pub struct ProxyResponse {
    status: u16,
    headers: HashMap<String, String>,
    body: String,
}

impl UserData for ProxyResponse {
    fn add_methods<'lua, T: rlua::UserDataMethods<'lua, Self>>(methods: &mut T) {
        methods.add_method("body", |_, res, ()| Ok(res.body.to_string()));
        methods.add_method("headers", |_, res, ()| Ok(res.headers.clone()));
        methods.add_method("status", |_, res, ()| Ok(res.status));

        methods.add_method_mut("set_body", |_, res, (body,): (String,)| {
            res.headers
                .insert("content-length".to_string(), body.len().to_string());
            res.body = body;
            Ok(())
        });
        methods.add_method_mut("set_status", |_, res, (status,)| {
            res.status = status;
            Ok(())
        });
        methods.add_method_mut("set_header", |_, res, (key, value)| {
            let _ = res.headers.insert(key, value);
            Ok(())
        });
    }
}

impl ProxyResponse {
    pub async fn from(response: Response<Body>) -> Result<Self, hyper::Error> {
        let (parts, body) = response.into_parts();

        let headers = parts
            .headers
            .iter()
            .map(|(key, val)| {
                (
                    key.to_string(),
                    String::from_utf8_lossy(val.as_bytes()).to_string(),
                )
            })
            .collect();

        let body = &to_bytes(body).await?;
        let body = String::from_utf8_lossy(body);

        Ok(ProxyResponse {
            status: parts.status.as_u16(),
            body: body.to_string(),
            headers,
        })
    }

    pub fn into_response(self) -> anyhow::Result<Response<Body>> {
        let mut response = Response::builder().status(self.status);

        for (key, value) in self.headers {
            response = response.header(key.as_str(), value.as_str());
        }

        Ok(response.body(Body::from(self.body))?)
    }

    pub fn _new(status: u16, body: &str) -> Self {
        Self {
            status,
            headers: Default::default(),
            body: body.to_string(),
        }
    }

    pub fn _with_header(&mut self, key: &str, val: &str) {
        let _ = self.headers.insert(key.to_string(), val.to_string());
    }
}
