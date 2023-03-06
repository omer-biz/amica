use std::collections::HashMap;

use hyper::{body::to_bytes, Body, Request};
use rlua::UserData;

#[derive(Clone)]
pub(crate) struct ProxyRequest {
    uri: String,
    method: String,
    headers: HashMap<String, String>,
    body: String,
}

impl ProxyRequest {
    pub async fn new(request: Request<Body>) -> Self {
        // request.version();
        let (parts, body) = request.into_parts();
        let headers: HashMap<String, String> = parts
            .headers
            .iter()
            .map(|header| (header.0.to_string(), header.1.to_str().unwrap().to_string()))
            .collect();
        let body = &to_bytes(body).await.unwrap();
        let body = String::from_utf8_lossy(body);

        ProxyRequest {
            uri: parts.uri.to_string(),
            method: parts.method.to_string(),
            body: body.to_string(),
            headers,
        }
    }
}

impl Into<Request<Body>> for ProxyRequest {
    fn into(self) -> Request<Body> {
        let mut request = Request::builder()
            .method(self.method.as_str())
            .uri(self.uri.as_str());

        for (key, value) in self.headers {
            request = request.header(key.as_str(), value.as_str());
        }

        request.body(Body::from(self.body)).unwrap()
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
        methods.add_method_mut("set_body", |_, req, (body,)| {
            req.body = body;
            Ok(())
        });
        methods.add_method_mut("set_headers", |_, req, (headers,)| {
            req.headers = headers;
            Ok(())
        });
    }
}
