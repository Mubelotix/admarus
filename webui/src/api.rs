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

pub async fn search(query: impl AsRef<str>) -> Result<u64, ApiError> {
    #[derive(Deserialize)]
    struct Rep {
        id: u64,
    }
    let rep: Rep = get(format!("http://localhost:3030/search?q={}", query.as_ref())).await?;
    Ok(rep.id)
}

pub async fn fetch_results(id: u64) -> Result<Vec<(DocumentResult, String)>, ApiError> {

    let rep: Vec<(DocumentResult, String)> = get(format!("http://localhost:3030/fetch-results?id={id}")).await?;
    Ok(rep)
}
