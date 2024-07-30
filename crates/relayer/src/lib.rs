use reqwest::Url;

mod sync;

pub struct Relayer {
    endpoint: Url,
}
