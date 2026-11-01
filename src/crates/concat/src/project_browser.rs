// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pure project-library filtering, ordering and page boundaries.
use concat_host::projects;

/// The projects in the order they were last opened, newest first.
pub const SORT_RECENT: i32 = 0;
/// The projects by name, A to Z.
pub const SORT_NAME: i32 = 1;

/// The recents the grid shows: those whose name or folder holds every
/// word of the query, ignoring case, newest first or by name. A missing
/// folder is found and sorted like any other.
pub fn shown<'a>(
    recents: &'a [projects::Recent],
    query: &str,
    sort: i32,
) -> Vec<&'a projects::Recent> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut shown: Vec<&projects::Recent> = recents
        .iter()
        .filter(|recent| {
            if words.is_empty() {
                return true;
            }
            let name = recent.info.name.to_lowercase();
            let path = recent.info.path.to_lowercase();
            words
                .iter()
                .all(|word| name.contains(word.as_str()) || path.contains(word.as_str()))
        })
        .collect();
    if sort == SORT_NAME {
        shown.sort_by_cached_key(|recent| recent.info.name.to_lowercase());
    }
    shown
}

/// Maximum number of projects published to the UI and thumbnail queue.
pub const PAGE_SIZE: usize = 24;

/// Clamp a requested zero-based page, retaining one empty page for no matches.
pub fn page_bounds(count: usize, requested: usize) -> (usize, usize) {
    let pages = count.div_ceil(PAGE_SIZE).max(1);
    (requested.min(pages - 1), pages)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn searching_and_sorting_precede_page_selection() {
        let recents = (0..105)
            .rev()
            .map(|i| projects::Recent {
                info: projects::ProjectInfo {
                    path: format!("/projects/{i}"),
                    name: format!("Lesson {i:03}"),
                    width: 64,
                    height: 36,
                    canvas: [0, 0, 0, 255],
                    rate_num: 30,
                    rate_den: 1,
                    opened_at: i,
                },
                present: true,
                edited_at: None,
            })
            .collect::<Vec<_>>();
        let matches = shown(&recents, "LESSON 09", SORT_NAME);
        assert_eq!(matches.len(), 11);
        let (page, pages) = page_bounds(matches.len(), 4);
        assert_eq!((page, pages), (0, 1));
        assert_eq!(matches[0].info.name, "Lesson 009");
        let all = shown(&recents, "", SORT_RECENT);
        let (page, pages) = page_bounds(all.len(), 4);
        assert_eq!((page, pages), (4, 5));
        assert_eq!(all.iter().skip(page * PAGE_SIZE).take(PAGE_SIZE).count(), 9);
        assert_eq!(page_bounds(96, page), (3, 4));
        assert_eq!(page_bounds(0, page), (0, 1));
    }
}
