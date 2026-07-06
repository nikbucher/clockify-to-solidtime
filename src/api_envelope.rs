use serde::{Deserialize, Deserializer};

pub(crate) fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
	D: Deserializer<'de>,
	T: Default + Deserialize<'de>,
{
	Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

pub(crate) fn blank_string_as_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
	D: Deserializer<'de>,
{
	Ok(Option::<String>::deserialize(deserializer)?.and_then(|value| {
		let value = value.trim().to_string();
		if value.is_empty() { None } else { Some(value) }
	}))
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ListEnvelope<T> {
	Direct(Vec<T>),
	Data {
		data: Vec<T>,
		links: Option<PaginationLinks>,
		meta: Option<PaginationMeta>,
	},
}

impl<T> ListEnvelope<T> {
	pub fn into_page(self) -> ListPage<T> {
		match self {
			Self::Direct(items) => ListPage {
				items,
				has_next: false,
				has_pagination_metadata: false,
			},
			Self::Data { data: items, links, meta } => {
				let has_next = links.as_ref().and_then(|links| links.next.as_deref()).is_some_and(|next| !next.is_empty()) || meta.as_ref().is_some_and(|meta| meta.has_next());
				ListPage {
					items,
					has_next,
					has_pagination_metadata: links.is_some() || meta.is_some(),
				}
			}
		}
	}
}

#[derive(Debug)]
pub struct ListPage<T> {
	pub items: Vec<T>,
	pub has_next: bool,
	pub has_pagination_metadata: bool,
}

#[derive(Debug, Deserialize)]
pub struct PaginationLinks {
	pub next: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PaginationMeta {
	pub current_page: Option<u64>,
	pub last_page: Option<u64>,
	pub next_page_url: Option<String>,
}

impl PaginationMeta {
	fn has_next(&self) -> bool {
		self.next_page_url.as_deref().is_some_and(|next| !next.is_empty()) || self.current_page.zip(self.last_page).is_some_and(|(current, last)| current < last)
	}
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ItemEnvelope<T> {
	Direct(T),
	Data { data: T },
}

impl<T> ItemEnvelope<T> {
	pub fn into_item(self) -> T {
		match self {
			Self::Direct(item) | Self::Data { data: item } => item,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn direct_list_has_no_pagination_metadata() {
		let envelope = ListEnvelope::Direct(vec![1, 2]);

		let page = envelope.into_page();

		assert_eq!(page.items, [1, 2]);
		assert!(!page.has_next);
		assert!(!page.has_pagination_metadata);
	}

	#[test]
	fn data_list_uses_links_next_url_for_has_next() {
		let page = ListEnvelope::Data {
			data: vec![1],
			links: Some(PaginationLinks {
				next: Some("https://example.test/next".to_string()),
			}),
			meta: None,
		}
		.into_page();

		assert!(page.has_next);
		assert!(page.has_pagination_metadata);
	}

	#[test]
	fn data_list_ignores_empty_links_next_url() {
		let page = ListEnvelope::Data {
			data: vec![1],
			links: Some(PaginationLinks { next: Some(String::new()) }),
			meta: None,
		}
		.into_page();

		assert!(!page.has_next);
		assert!(page.has_pagination_metadata);
	}

	#[test]
	fn data_list_uses_meta_next_page_url_for_has_next() {
		let page = ListEnvelope::Data {
			data: vec![1],
			links: None,
			meta: Some(PaginationMeta {
				current_page: None,
				last_page: None,
				next_page_url: Some("https://example.test/next".to_string()),
			}),
		}
		.into_page();

		assert!(page.has_next);
		assert!(page.has_pagination_metadata);
	}

	#[test]
	fn data_list_uses_meta_page_numbers_for_has_next() {
		let page = ListEnvelope::Data {
			data: vec![1],
			links: None,
			meta: Some(PaginationMeta {
				current_page: Some(1),
				last_page: Some(2),
				next_page_url: None,
			}),
		}
		.into_page();

		assert!(page.has_next);
		assert!(page.has_pagination_metadata);
	}

	#[test]
	fn data_list_has_no_next_when_links_and_meta_do_not_point_to_next_page() {
		let page = ListEnvelope::Data {
			data: vec![1],
			links: Some(PaginationLinks { next: None }),
			meta: Some(PaginationMeta {
				current_page: Some(2),
				last_page: Some(2),
				next_page_url: None,
			}),
		}
		.into_page();

		assert!(!page.has_next);
		assert!(page.has_pagination_metadata);
	}
}
