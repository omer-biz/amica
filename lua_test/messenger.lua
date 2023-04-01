-- .body(Body::from("Good Bye, World"))?;

function on_http_request(req)
  req:set_method("POST")
  req:set_uri("http://www.example.com")
  req:set_header("header1", "changed_value1")
  req:set_header("header2", "changed_value2")
  req:set_header("header3", "changed_value3")
  req:set_header("new_header", "new_header")
  req:set_body("Good Bye, World")
  return req
end

function on_http_response(res)
  res:set_status(401)
  res:set_header("header1", "changed_value1")
  res:set_header("header2", "changed_value2")
  res:set_header("header3", "changed_value3")
  res:set_header("new_header", "new_header")
  res:set_body("Good Bye, World")
  return res
end
