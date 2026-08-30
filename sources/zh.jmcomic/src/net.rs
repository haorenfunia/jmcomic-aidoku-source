use aidoku::{
	Result,
	alloc::{String, format},
	imports::{html::Document, net::Request},
	prelude::error,
};
use base64::{Engine, engine::general_purpose::STANDARD};

use crate::settings;

pub const JM_UA: &str = "Mozilla/5.0 (Linux; Android 10; K; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/130.0.0.0 Mobile Safari/537.36";
pub const JM_PKG: &str = "com.example.app";
pub const DEFAULT_DOMAIN: &str = "18comic.ink";

pub struct ApiContext {
	pub domain: String,
}

impl ApiContext {
	pub fn base_url(&self) -> String {
		format!("https://{}", self.domain)
	}

	pub fn get_html(&self, path: &str) -> Result<Document> {
		let url = if path.starts_with("http") {
			path.into()
		} else {
			format!("https://{}{}", self.domain, path)
		};
		Request::get(&url)?
			.header("user-agent", JM_UA)
			.header("referer", &format!("https://{}/", self.domain))
			.header("accept-language", "zh-CN,zh;q=0.9,en-US;q=0.8,en;q=0.7")
			.html()
			.map_err(|_| error!("页面加载失败：{}", path))
	}

	pub fn get_album(&self, id: &str) -> Result<Document> {
		let document = self.get_html(&url::album(id))?;
		let Some(scripts) = document
			.select("#wrapper > script:containsData(function base64DecodeUtf8):containsData(document.write(html))")
		else {
			return Ok(document);
		};
		for script in scripts {
			let Some(code) = script.html() else {
				continue;
			};
			let Some(start) = code.find("base64DecodeUtf8(\"") else {
				continue;
			};
			let start = start + "base64DecodeUtf8(\"".len();
			let Some(end) = code[start..].find("\");") else {
				continue;
			};
			let encoded = &code[start..start + end];
			let Ok(decoded) = STANDARD.decode(encoded.as_bytes()) else {
				continue;
			};
			let Ok(fragment) = core::str::from_utf8(&decoded) else {
				continue;
			};
			if let Some(mut body) = document.select_first("body") {
				let _ = body.append(fragment);
			}
		}
		Ok(document)
	}
}

pub fn context() -> Result<ApiContext> {
	let domain = settings::mirror_domain()
		.and_then(|value| normalize_domain(&value))
		.unwrap_or_else(|| DEFAULT_DOMAIN.into());
	Ok(ApiContext { domain })
}

pub fn normalize_domain(domain: &str) -> Option<String> {
	let value = domain
		.trim()
		.trim_start_matches("https://")
		.trim_start_matches("http://")
		.trim_end_matches('/');
	(!value.is_empty()).then(|| value.into())
}

pub mod url {
	use aidoku::{
		alloc::{String, format},
		helpers::uri::encode_uri_component,
	};

	pub fn album(id: &str) -> String {
		format!("/album/{id}")
	}

	pub fn photo(id: &str) -> String {
		format!("/photo/{id}")
	}

	pub fn listing(category: &str, sort: &str, time: &str, scope: &str, page: i32) -> String {
		let base = match category {
			"another" => "/albums/another?",
			"doujin" => "/albums/doujin?",
			"hanman" => "/albums/hanman?",
			"meiman" => "/albums/meiman?",
			"short" => "/albums/short?",
			"single" => "/albums/single?",
			"chinese" => "/albums/doujin/sub/chinese?",
			"japanese" => "/albums/doujin/sub/japanese?",
			"cosplay" => "/albums/doujin/sub/cosplay?",
			"cg" => "/albums/doujin/sub/CG?",
			_ => "/albums?",
		};
		format!("{base}o={sort}&t={time}&main_tag={scope}&page={page}")
	}

	pub fn tag_search(tag: &str, sort: &str, time: &str, scope: &str, page: i32) -> String {
		format!(
			"/search/photos?search_query={}&page={page}&o={sort}&t={time}&main_tag={scope}",
			encode_uri_component(tag),
		)
	}

	pub fn text_search(query: &str, sort: &str, time: &str, scope: &str, page: i32) -> String {
		let query = query.replace('+', "%2B").replace(' ', "+");
		format!(
			"/search/photos?search_query={query}&page={page}&o={sort}&t={time}&main_tag={scope}"
		)
	}
}
