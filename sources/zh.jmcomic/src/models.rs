use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	imports::html::Element,
	prelude::format,
};

pub struct BlockState {
	keywords: Vec<String>,
	ids: Vec<String>,
}

impl BlockState {
	pub fn new(entries: Vec<String>) -> Self {
		let mut keywords = Vec::new();
		let mut ids = Vec::new();
		for entry in entries {
			if entry.chars().all(|c| c.is_ascii_digit()) {
				ids.push(entry);
			} else {
				keywords.push(entry.to_lowercase());
			}
		}
		Self { keywords, ids }
	}

	pub fn is_blocked(&self, id: &str, fields: impl IntoIterator<Item = String>) -> bool {
		let fields = fields.into_iter().collect::<Vec<_>>();
		self.ids.iter().any(|value| value == id)
			|| self.keywords.iter().any(|keyword| {
				fields
					.iter()
					.any(|field| field.to_lowercase().contains(keyword.as_str()))
			})
	}
}

pub struct ComicItem {
	pub id: String,
	pub title: String,
	pub author: Option<String>,
	pub image: Option<String>,
	pub tags: Vec<String>,
}

impl ComicItem {
	pub fn from_element(element: &Element) -> Option<Self> {
		let link = element.select_first("a[href^=/album/]")?;
		let href = link.attr("href")?;
		let id = href
			.split("/album/")
			.nth(1)?
			.split(['/', '?', '#'])
			.next()?
			.to_string();
		if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
			return None;
		}

		let image = element
			.select_first("img")
			.and_then(|img| {
				img.attr("abs:data-original")
					.or_else(|| img.attr("abs:data-cfsrc"))
					.or_else(|| img.attr("abs:data-src"))
					.or_else(|| img.attr("abs:src"))
			})
			.filter(|url| !url.contains("/blank.jpg"));
		let title = element
			.select_first(".video-title")
			.and_then(|node| node.text())
			.or_else(|| element.select_first("img").and_then(|img| img.attr("title")))
			.unwrap_or_default()
			.trim()
			.into();
		if title.is_empty() {
			return None;
		}

		let author = element
			.select_first("div.title-truncate:not(.tags)")
			.and_then(|node| node.select("a"))
			.map(|nodes| {
				nodes
					.filter_map(|node| node.text())
					.map(|text| text.trim().into())
					.collect::<Vec<String>>()
					.join(", ")
			})
			.filter(|value| !value.is_empty());
		let tags = element
			.select("div.title-truncate.tags a")
			.into_iter()
			.flatten()
			.filter_map(|node| node.text())
			.map(|text| text.trim().into())
			.filter(|text: &String| !text.is_empty())
			.collect();

		Some(Self {
			id,
			title,
			author,
			image,
			tags,
		})
	}

	pub fn is_blocked(&self, block: &BlockState) -> bool {
		block.is_blocked(
			&self.id,
			core::iter::once(self.title.clone())
				.chain(self.author.clone())
				.chain(self.tags.iter().cloned()),
		)
	}

	pub fn into_manga(self, base_url: &str) -> Manga {
		Manga {
			key: self.id.clone(),
			title: self.title,
			cover: self.image,
			authors: self.author.map(|author| vec![author]),
			url: Some(format!("{base_url}/album/{}", self.id)),
			content_rating: ContentRating::NSFW,
			..Default::default()
		}
	}
}

#[derive(Clone)]
pub struct AlbumData {
	pub title: String,
	pub cover: Option<String>,
	pub authors: Vec<String>,
	pub description: Option<String>,
	pub tags: Vec<String>,
	pub chapters: Vec<ChapterEntry>,
	pub single_chapter: Option<ChapterEntry>,
}

#[derive(Clone)]
pub struct ChapterEntry {
	pub key: String,
	pub title: String,
	pub date_uploaded: Option<i64>,
	pub url: String,
}

impl AlbumData {
	pub fn from_document(document: &aidoku::imports::html::Document, base_url: &str) -> Self {
		let title = document
			.select_first("h1")
			.and_then(|node| node.text())
			.unwrap_or_default()
			.trim()
			.into();
		let cover = document.select_first("#album_photo_cover img").and_then(|img| {
			img.attr("abs:data-original")
				.or_else(|| img.attr("abs:data-cfsrc"))
				.or_else(|| img.attr("abs:data-src"))
				.or_else(|| img.attr("abs:src"))
				.filter(|url| !url.contains("/blank.jpg"))
		});
		let authors = document
			.select_first("span[itemprop=author][data-type=author]")
			.and_then(|node| node.select("a"))
			.into_iter()
			.flatten()
			.filter_map(|node| node.text())
			.map(|text| text.trim().into())
			.filter(|text: &String| !text.is_empty())
			.collect();
		let tags = document
			.select_first("span[itemprop=genre]")
			.and_then(|node| node.select("a"))
			.into_iter()
			.flatten()
			.filter_map(|node| node.text())
			.map(|text| text.trim().into())
			.filter(|text: &String| !text.is_empty())
			.collect();
		let description = document
			.select_first("#intro-block .p-t-5.p-b-5")
			.and_then(|node| node.text())
			.map(|text| text.trim().trim_start_matches("敘述：").trim().into())
			.filter(|text: &String| !text.is_empty());

		let chapters = document
			.select("div[id=episode-block] a[href^=/photo/]")
			.into_iter()
			.flatten()
			.filter_map(|node| chapter_from_element(&node, base_url))
			.collect::<Vec<_>>();
		let single_chapter = if chapters.is_empty() {
			document
				.select_first("#album_photo_cover > div.thumb-overlay > a")
				.and_then(|node| chapter_from_element(&node, base_url))
		} else {
			None
		};

		Self {
			title,
			cover,
			authors,
			description,
			tags,
			chapters,
			single_chapter,
		}
	}

	pub fn into_manga(self, key: &str, base_url: &str) -> Manga {
		let status = if self.tags.iter().any(|tag| tag == "完結" || tag == "已完結") {
			MangaStatus::Completed
		} else if self.tags.iter().any(|tag| tag == "連載中" || tag == "連載") {
			MangaStatus::Ongoing
		} else {
			MangaStatus::Unknown
		};
		let viewer = if self.tags.iter().any(|tag| {
			tag == "條漫" || tag == "条漫" || tag == "韩漫" || tag == "韓漫" || tag == "一般向韓漫" || tag.eq_ignore_ascii_case("webtoon")
		}) {
			Viewer::Webtoon
		} else if self.tags.iter().any(|tag| tag == "美漫" || tag == "英文") {
			Viewer::LeftToRight
		} else {
			Viewer::default()
		};
		Manga {
			key: key.into(),
			title: self.title,
			cover: self.cover,
			authors: (!self.authors.is_empty()).then_some(self.authors),
			description: self.description,
			tags: (!self.tags.is_empty()).then_some(self.tags),
			url: Some(format!("{base_url}/album/{key}")),
			status,
			viewer,
			content_rating: ContentRating::NSFW,
			..Default::default()
		}
	}

	pub fn to_chapters(self, manga_key: &str, base_url: &str) -> Vec<Chapter> {
		let mut entries = self.chapters;
		if entries.is_empty() {
			if let Some(entry) = self.single_chapter {
				entries.push(entry);
			} else {
				entries.push(ChapterEntry {
					key: manga_key.into(),
					title: "单章节".into(),
					date_uploaded: None,
					url: format!("{base_url}/photo/{manga_key}"),
				});
			}
		}
		entries.reverse();
		entries
			.into_iter()
			.enumerate()
			.map(|(index, entry)| Chapter {
				key: entry.key,
				title: Some(entry.title),
				chapter_number: Some((index + 1) as f32),
				date_uploaded: entry.date_uploaded,
				url: Some(entry.url),
				..Default::default()
			})
			.collect()
	}
}

fn chapter_from_element(element: &Element, base_url: &str) -> Option<ChapterEntry> {
	let href = element.attr("abs:href")?;
	let key = href
		.split("/photo/")
		.nth(1)?
		.split(['/', '?', '#'])
		.next()?
		.to_string();
	if key.is_empty() {
		return None;
	}
	let title = element
		.select_first("li h3")
		.and_then(|node| node.text())
		.or_else(|| element.select_first("h3").and_then(|node| node.text()))
		.unwrap_or_else(|| "章节".into())
		.trim()
		.into();
	let date_uploaded = element
		.select_first("li span.hidden-xs")
		.and_then(|node| node.text())
		.and_then(|date| aidoku::imports::std::parse_date(date.trim(), "yyyy-MM-dd"));
	Some(ChapterEntry {
		key,
		title,
		date_uploaded,
		url: if href.starts_with("http") {
			href
		} else {
			format!("{base_url}{href}")
		},
	})
}
