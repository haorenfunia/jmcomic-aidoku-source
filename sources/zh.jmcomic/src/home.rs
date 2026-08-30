use aidoku::{
	FilterItem, FilterValue, Home, HomeComponent, HomeComponentValue, HomeLayout, Listing, Manga,
	Result, alloc::vec,
};

use crate::{JMComic, block_ctx, net, parse_list};

impl Home for JMComic {
	fn get_home(&self) -> Result<HomeLayout> {
		let api = net::context()?;
		let block = block_ctx();
		let korean = parse_list(
			&api,
			&net::url::listing("hanman", "mr", "a", "0", 1),
			&block,
		)
		.unwrap_or_default();

		Ok(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("筛选器".into()),
					subtitle: None,
					value: HomeComponentValue::Filters(vec![
						filter_item("韩漫追新", "hanman"),
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
				id: "category".into(),
				value: category.into(),
			},
			FilterValue::Select {
				id: "sort".into(),
				value: "mr".into(),
			},
			FilterValue::Select {
				id: "time".into(),
				value: "a".into(),
			},
			FilterValue::Select {
				id: "scope".into(),
				value: "0".into(),
			},
		]),
	}
}
