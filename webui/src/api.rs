use crate::prelude::*;

#[derive(Debug)]
pub enum ApiError {
    InputJson(serde_json::Error),
    OutputJson(serde_json::Error),
    Fetch(JsValue),
    NotText(JsValue),
    BadRequest(String),
    Server(String),
    Unknown(String),
}

impl ApiError {
    pub fn to_format_parts(&self) -> (&'static str, Vec<String>, String) {
        let (title, recommandations, details) = match self {
            ApiError::InputJson(e) => (
                "Failed to craft request",
                vec![
                    String::from("Open an issue on GitHub"),
                    String::from("Try again"),
                ],
                format!("InputJson: {e}")
            ),
            ApiError::OutputJson(e) => (
                "Failed to read results",
                vec![
                    String::from("Make sure your daemon is up to date"),
                    String::from("Make sure the daemon address is correct"),
                    String::from("Open an issue on GitHub"),
                    String::from("Try again"),
                ],
                format!("OutputJson: {e}")
            ),
            ApiError::Fetch(e) => (
                "Failed to send query",
                vec![
                    String::from("Make sure the daemon is running"),
                    String::from("Make sure the daemon address is correct"),
                    String::from("Make sure CORS is properly configured"),
                    String::from("Try again"),
                ],
                format!("Fetch: {}", e.clone().dyn_into::<js_sys::Error>().unwrap().message())
            ),
            ApiError::NotText(e) => (
                "Invalid response",
                vec![
                    String::from("Make sure the daemon address is correct"),
                    String::from("Open an issue on GitHub"),
                    String::from("Try again"),
                ],
                format!("NotText: {}", e.clone().dyn_into::<js_sys::Error>().unwrap().message())
            ),
            ApiError::BadRequest(e) => (
                "Failed to communicate with daemon",
                vec![
                    String::from("Make sur the daemon is up to date"),
                    String::from("Make sure the daemon address is correct"),
                    String::from("Open an issue on GitHub"),
                    String::from("Try again"),
                ],
                format!("BadRequest: {e}")
            ),
            ApiError::Server(e) => (
                "Daemon is having issues",
                vec![
                    String::from("Make sure the daemon is up to date"),
                    String::from("Open an issue on GitHub"),
                    String::from("Try again"),
                ],
                format!("Server: {e}")
            ),
            ApiError::Unknown(e) => (
                "Unknown error",
                vec![
                    String::from("Make sure the daemon address is correct"),
                    String::from("Try again"),
                ],
                format!("Unknown: {e}")
            ),
        };
        (title, recommandations, details)
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(e: serde_json::Error) -> Self {
        ApiError::OutputJson(e)
    }
}

async fn get<T: DeserializeOwned>(url: impl AsRef<str>) -> Result<T, ApiError> {
    api_custom_method(url, "GET", ()).await
}

async fn api_custom_method<I: Serialize, O: DeserializeOwned>(endpoint: impl AsRef<str>, method: &'static str, body: I) -> Result<O, ApiError> {
    use ApiError::*;

    let mut req_init = RequestInit::new();
    req_init.method(method);

    let request = Request::new_with_str_and_init(endpoint.as_ref(), &req_init).unwrap();
    /*request.headers().set(
        "Api-Key",
        &format!("{}-{}-{}", api_key, counter, gen_code(api_key, counter)),
    )?;*/
    if std::any::type_name::<I>() != "()" {
        request.headers().set("Content-Type", "application/json").expect("Failed to set content type");
        let body = serde_json::to_string(&body).map_err(InputJson)?;
        let body = JsValue::from_str(&body);
        req_init.body(Some(&body));
    }

    let promise = wndw().fetch_with_request(&request);
    let future = JsFuture::from(promise);
    let response = future.await.map_err(ApiError::Fetch)?;
    let response: Response = response.dyn_into().expect("response isn't a Response");
    let text = JsFuture::from(response.text().map_err(NotText)?).await.map_err(NotText)?;
    let text: String = text.as_string().expect("text isn't a string");

    match response.status() {
        200 => {
            if std::any::type_name::<O>() == "()" && text.is_empty() {
                return Ok(serde_json::from_str("null").unwrap());
            }
            serde_json::from_str(&text).map_err(OutputJson)
        }
        400 => Err(BadRequest(text)),
        500 => Err(Server(text)),
        _ => Err(Unknown(text))
    }
}

pub async fn check_ipfs(ipfs_addr: &str) -> Result<(), ApiError> {
    use ApiError::*;

    let url = ipfs_addr.replace("://", "://bafybeigmfwlweiecbubdw4lq6uqngsioqepntcfohvrccr2o5f7flgydme.ipfs.");
    let request = Request::new_with_str(&url).unwrap();
    let promise = wndw().fetch_with_request(&request);
    let future = JsFuture::from(promise);
    let response = future.await.map_err(ApiError::Fetch)?;
    let response: Response = response.dyn_into().expect("response isn't a Response");
    let text = JsFuture::from(response.text().map_err(NotText)?).await.map_err(NotText)?;
    let text: String = text.as_string().expect("text isn't a string");

    if response.status() == 200 && text == "Hello World!\r\n"  {
        Ok(())
    } else {
        Err(ApiError::Server(String::from("Failed to fetch example CID")))
    }
}

pub async fn search(rpc_addr: &str, query: impl AsRef<str>) -> Result<ApiSearchResponse, ApiError> {
    get(format!("{rpc_addr}/search?q={}", url_encode(query.as_ref()))).await
}

pub async fn fetch_results(rpc_addr: &str, id: u64) -> Result<Vec<(DocumentResult, String)>, ApiError> {
    get(format!("{rpc_addr}/results?id={id}")).await
}

pub async fn get_result(rpc_addr: &str, id: u64, cid: &str) -> Result<Option<DocumentResult>, ApiError> {
    get(format!("{rpc_addr}/result?id={id}&cid={cid}")).await
}

pub async fn get_api_version(rpc_addr: &str) -> Result<u64, ApiError> {
    get::<ApiVersionResponse>(format!("{rpc_addr}/version")).await.map(|r| r.version)
}
