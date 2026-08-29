use aidoku::{
	FilterItem, FilterValue, Home, HomeComponent, HomeComponentValue, HomeLayout, Listing,
	ListingProvider, Manga, MangaPageResult, Result,
	alloc::vec,
};

use crate::{JMComic, block_ctx, net, search_result};

const KOREAN_CATEGORY: &str = "hanman";

impl ListingProvider for JMComic {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let api = net::context()?;
		let block = block_ctx(None);
		let page = page.max(1);
		match listing.id.as_str() {
			"korean-latest" => search_result(
				&api,
				&net::url::filter("mr", KOREAN_CATEGORY, page),
				&block,
				"",
			),
			"latest" => search_result(
				&api,
				&net::url::filter("mr", "", page),
				&block,
				"",
			),
			_ => Ok(MangaPageResult::default()),
		}
	}
}

impl Home for JMComic {
	fn get_home(&self) -> Result<HomeLayout> {
		let api = net::context()?;
		let block = block_ctx(None);
		let korean = search_result(
			&api,
			&net::url::filter("mr", KOREAN_CATEGORY, 1),
			&block,
			"",
		)?;

		Ok(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("筛选器".into()),
					subtitle: None,
					value: HomeComponentValue::Filters(vec![
						filter_item("韩漫追新", KOREAN_CATEGORY),
						filter_item("日漫追新", "japanese"),
						filter_item("美漫追新", "meiman"),
						filter_item("全部最新", ""),
					]),
				},
				HomeComponent {
					title: Some("韩漫追新".into()),
					subtitle: None,
					value: HomeComponentValue::Scroller {
						entries: korean.entries.into_iter().map(Manga::into).collect(),
						listing: Some(Listing {
							id: "korean-latest".into(),
							name: "韩漫追新".into(),
							..Default::default()
						}),
					},
				},
			],
		})
	}
}

fn filter_item(title: &str, category: &str) -> FilterItem {
	FilterItem {
		title: title.into(),
		values: Some(vec![
			FilterValue::Select {
				id: "language".into(),
				value: category.into(),
			},
			FilterValue::Select {
				id: "sort".into(),
				value: "mr".into(),
			},
		]),
	}
}
