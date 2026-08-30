#![no_std]

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, HashMap, ImageRequestProvider,
	ImageResponse, Listing, ListingProvider, Manga, MangaPageResult, Page, PageContent, PageContext,
	PageImageProcessor, Result, Source,
	alloc::{String, Vec, format, vec},
	canvas::Rect,
	helpers::uri::encode_uri_component,
	imports::{canvas::{Canvas, ImageRef}, net::Request},
	prelude::*,
};

mod home;
mod models;
mod net;
mod settings;

use models::{AlbumData, BlockState, ComicItem};
use net::ApiContext;

const SCRAMBLE_ID: u64 = 220980;

struct JMComic;

impl Source for JMComic {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let api = net::context()?;
		let block = block_ctx();
		let page = page.max(1);
		let query = query.filter(|value| !value.trim().is_empty());

		if let Some(value) = query.as_deref()
			&& let Some(key) = parse_manga_key(value)
		{
			return direct_manga_result(&api, &key, &block);
		}

		let (category, sort, time, scope) = parse_filters(&filters);
		let path = build_search_path(query.as_deref(), &category, sort, time, scope, page);
		parse_list(&api, &path, &block)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let api = net::context()?;
		let document = api.get_album(&manga.key)?;
		let data = AlbumData::from_document(&document, &api.base_url());
		if data.title.is_empty() {
			bail!("漫画不存在或页面被拦截");
		}
		let chapters = needs_chapters.then(|| data.clone().to_chapters(&manga.key, &api.base_url()));
		if needs_details {
			manga.copy_from(data.into_manga(&manga.key, &api.base_url()));
		}
		if let Some(chapters) = chapters {
			manga.chapters = Some(chapters);
		}
		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let api = net::context()?;
		let block = block_ctx();
		if block.is_blocked(&manga.key, core::iter::once(manga.title.clone())) {
			bail!("这个内容已经被你屏蔽啦");
		}

		let first = chapter
			.url
		.as_deref()
			.and_then(|url| url.find("/photo/").map(|index| &url[index..]))
			.map(String::from)
			.unwrap_or_else(|| net::url::photo(&chapter.key));
		let mut pages = Vec::new();
		let mut next = Some(first);
		let mut count = 0;
		while let Some(path) = next.take() {
			if count >= 100 {
				break;
			}
			count += 1;
			let document = api.get_html(&path)?;
			parse_pages(&document, &mut pages);
			next = document
			.select_first("a.prevnext")
			.and_then(|node| node.attr("abs:href"))
			.map(|url| url.into());
		}
		if pages.is_empty() {
			bail!("章节没有找到图片");
		}
		Ok(pages)
	}
}

impl ListingProvider for JMComic {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let api = net::context()?;
		let block = block_ctx();
		let path = match listing.id.as_str() {
			"korean-latest" => net::url::listing("hanman", "mr", "a", "0", page.max(1)),
			"japanese-latest" => net::url::listing("japanese", "mr", "a", "0", page.max(1)),
			"western-latest" => net::url::listing("meiman", "mr", "a", "0", page.max(1)),
			"all-latest" => net::url::listing("", "mr", "a", "0", page.max(1)),
			id if id.starts_with("category:") => net::url::listing(&id[9..], "mr", "a", "0", page.max(1)),
			id if id.starts_with("tag:") => net::url::tag_search(&id[4..], "mr", "a", "0", page.max(1)),
			_ => return Ok(MangaPageResult::default()),
		};
		parse_list(&api, &path, &block)
	}
}

impl PageImageProcessor for JMComic {
	fn process_page_image(
		&self,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		let Some(context) = context.as_ref() else {
			return Ok(response.image);
		};
		let ep_id = context.get("ep_id").and_then(|value| value.parse().ok()).unwrap_or(0);
		let filename = context.get("filename").map(String::as_str).unwrap_or_default();
		let rows = scramble_rows(ep_id, filename);
		if rows <= 1 || filename.ends_with(".gif") {
			return Ok(response.image);
		}

		let width = response.image.width();
		let height = response.image.height();
		let height_px = height as u32;
		let remainder = height_px % rows;
		let base_height = height_px / rows;
		let mut canvas = Canvas::new(width, height);
		for index in 0..rows {
			let mut copy_height = base_height;
			let mut destination_y = base_height * index;
			let source_y = height_px - (base_height * (index + 1)) - remainder;
			if index == 0 {
				copy_height += remainder;
			} else {
				destination_y += remainder;
			}
			let h = copy_height as f32;
			canvas.copy_image(
				&response.image,
				Rect::new(0.0, source_y as f32, width, h),
				Rect::new(0.0, destination_y as f32, width, h),
			);
		}
		Ok(canvas.get_image())
	}
}

impl ImageRequestProvider for JMComic {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		let domain = settings::mirror_domain().unwrap_or_else(|| net::DEFAULT_DOMAIN.into());
		Ok(Request::get(url)?
			.header("referer", &format!("https://{domain}/"))
			.header("user-agent", net::JM_UA)
			.header("x-requested-with", net::JM_PKG))
	}
}

impl DeepLinkHandler for JMComic {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		Ok(extract_id(&url, "/album/").map(|key| DeepLinkResult::Manga { key }))
	}
}

pub(crate) fn parse_list(api: &ApiContext, path: &str, block: &BlockState) -> Result<MangaPageResult> {
	let document = api.get_html(path)?;
	let nodes = document
		.select("div.list-col")
		.ok_or_else(|| error!("列表页面解析失败"))?;
	let entries = nodes
		.filter_map(|node| ComicItem::from_element(&node))
		.filter(|item| !item.is_blocked(block))
		.map(|item| item.into_manga(&api.base_url()))
		.collect::<Vec<_>>();
	Ok(MangaPageResult {
		has_next_page: document.select_first("a.prevnext").is_some() && !entries.is_empty(),
		entries,
	})
}

fn parse_pages(document: &aidoku::imports::html::Document, pages: &mut Vec<Page>) {
	let Some(nodes) = document.select("div[class=center scramble-page spnotice_chk][id*=0]") else {
		return;
	};
	for node in nodes {
		let Some(image) = node.select_first("img") else {
			continue;
		};
		let src = image.attr("abs:src").unwrap_or_default();
		let cfsrc = image.attr("abs:data-cfsrc").unwrap_or_default();
		let original = image.attr("abs:data-original").unwrap_or_default();
		let image_url = if src.contains("blank.jpg") || cfsrc.contains("blank.jpg") {
			original
		} else {
			src
		};
		if image_url.is_empty() || image_url.contains("blank.jpg") {
			continue;
		}
		let (ep_id, filename) = image_context(&image_url);
		let mut context = HashMap::new();
		context.insert("ep_id".into(), ep_id);
		context.insert("filename".into(), filename);
		pages.push(Page {
			content: PageContent::url_context(image_url, context),
			..Default::default()
		});
	}
}

fn image_context(url: &str) -> (String, String) {
	let mut parts = url.split('/').rev();
	let filename = parts.next().unwrap_or_default().split('?').next().unwrap_or_default();
	let ep_id = parts.next().unwrap_or_default();
	(ep_id.into(), filename.into())
}

fn direct_manga_result(api: &ApiContext, key: &str, block: &BlockState) -> Result<MangaPageResult> {
	let document = api.get_album(key)?;
	let data = AlbumData::from_document(&document, &api.base_url());
	if data.title.is_empty() || block.is_blocked(key, core::iter::once(data.title.clone())) {
		return Ok(MangaPageResult::default());
	}
	Ok(MangaPageResult {
		has_next_page: false,
		entries: vec![data.into_manga(key, &api.base_url())],
	})
}

fn parse_filters(filters: &[FilterValue]) -> (&str, &str, &str, &str) {
	let mut category = "";
	let mut sort = "mr";
	let mut time = "a";
	let mut scope = "0";
	for filter in filters {
		if let FilterValue::Select { id, value } = filter {
			if value.is_empty() {
				continue;
			}
			match id.as_str() {
				"category" | "language" => category = value,
				"sort" => sort = value,
				"time" => time = value,
				"scope" | "type" => scope = value,
				_ => {}
			}
		}
	}
	(category, sort, time, scope)
}

fn build_search_path(
	query: Option<&str>,
	category: &str,
	sort: &str,
	time: &str,
	scope: &str,
	page: i32,
) -> String {
	if let Some(query) = query {
		if !query.split_whitespace().any(|value| value.starts_with('-')) {
			if let Some(tag) = category.strip_prefix("tag:") {
				let combined = format!("{}+%2B{}", query.replace('+', "%2B").replace(' ', "+"), encode_uri_component(tag));
				return format!(
					"/search/photos?search_query={combined}&page={page}&o={sort}&t={time}&main_tag={scope}"
				);
			}
			return net::url::text_search(query, sort, time, scope, page);
		}
	}

	let mut path = if let Some(tag) = category.strip_prefix("tag:") {
		net::url::tag_search(tag, sort, time, scope, page)
	} else {
		net::url::listing(category, sort, time, scope, page)
	};
	if let Some(query) = query {
		let excluded = query
			.split_whitespace()
			.filter_map(|value| value.strip_prefix('-'))
			.filter(|value| !value.is_empty())
			.map(|value| encode_uri_component(value))
			.collect::<Vec<_>>()
			.join("+");
		if !excluded.is_empty() {
			path.push_str("&screen=");
			path.push_str(&excluded);
		}
	}
	path
}

fn block_ctx() -> BlockState {
	BlockState::new(settings::blocked_entries())
}

fn parse_manga_key(value: &str) -> Option<String> {
	let value = value.trim();
	if value.chars().all(|c| c.is_ascii_digit()) && !value.is_empty() {
		return Some(value.into());
	}
	extract_id(value, "/album/")
}

fn extract_id(url: &str, marker: &str) -> Option<String> {
	let (_, tail) = url.split_once(marker)?;
	let id = tail
		.split(['/', '?', '#'])
		.next()
		.filter(|value| !value.is_empty() && value.chars().all(|c| c.is_ascii_digit()))?;
	Some(id.into())
}

fn scramble_rows(ep_id: u64, filename: &str) -> u32 {
	if ep_id < SCRAMBLE_ID {
		return 0;
	}
	if ep_id < 268850 {
		return 10;
	}
	let stem = filename.rsplit_once('.').map(|(name, _)| name).unwrap_or(filename);
	let digest = md5::compute(format!("{ep_id}{stem}").as_bytes());
	let last_byte = digest.0[15];
	let hex = format!("{last_byte:x}");
	let last_code = hex.as_bytes().last().copied().unwrap_or(b'0') as u32;
	let modulus = if ep_id >= 421926 { 8 } else { 10 };
	2 * (last_code % modulus) + 2
}

register_source!(
	JMComic,
	Home,
	ListingProvider,
	ImageRequestProvider,
	PageImageProcessor,
	DeepLinkHandler
);
