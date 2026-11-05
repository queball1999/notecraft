//! The note tag gallery: OneNote's default tags with the slugs they are stored under, and the
//! custom tags a person adds in the Customize Tags dialog (docs/markdown.md section 4).
//!
//! This is data only. The canvas draws each [`TagIcon`] in code; settings keep the custom
//! tags (the front end app saves [`SavedTag`] values).

use crate::tags::is_valid_name;

/// Most custom tags.
pub const MAX_CUSTOM_TAGS: usize = 50;
/// Longest tag display name, in characters.
pub const MAX_TAG_NAME_CHARS: usize = 40;
/// Longest slug made from a name, in bytes.
const MAX_SLUG_BYTES: usize = 32;

/// The drawing used for a tag's icon. Every one is drawn in code, so a custom tag chooses from
/// this set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TagIcon {
    Checkbox,
    Star,
    Question,
    /// A marker pen: the icon of the highlight-style tags.
    Marker,
    Person,
    Pin,
    Phone,
    Globe,
    Bulb,
    Key,
    Alert,
    Flag,
    Film,
    Book,
    Note,
    Quote,
    Pencil,
    Chat,
    Mail,
    Calendar,
    Callback,
    Briefcase,
    Heart,
    Dot,
}

impl TagIcon {
    /// Every icon, in the order the Customize Tags dialog lists them.
    pub const ALL: [TagIcon; 24] = [
        TagIcon::Dot,
        TagIcon::Star,
        TagIcon::Question,
        TagIcon::Alert,
        TagIcon::Checkbox,
        TagIcon::Marker,
        TagIcon::Bulb,
        TagIcon::Heart,
        TagIcon::Flag,
        TagIcon::Person,
        TagIcon::Pin,
        TagIcon::Phone,
        TagIcon::Callback,
        TagIcon::Mail,
        TagIcon::Calendar,
        TagIcon::Globe,
        TagIcon::Chat,
        TagIcon::Key,
        TagIcon::Film,
        TagIcon::Book,
        TagIcon::Note,
        TagIcon::Quote,
        TagIcon::Pencil,
        TagIcon::Briefcase,
    ];

    /// The stable name saved in settings.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Checkbox => "checkbox",
            Self::Star => "star",
            Self::Question => "question",
            Self::Marker => "marker",
            Self::Person => "person",
            Self::Pin => "pin",
            Self::Phone => "phone",
            Self::Globe => "globe",
            Self::Bulb => "bulb",
            Self::Key => "key",
            Self::Alert => "alert",
            Self::Flag => "flag",
            Self::Film => "film",
            Self::Book => "book",
            Self::Note => "note",
            Self::Quote => "quote",
            Self::Pencil => "pencil",
            Self::Chat => "chat",
            Self::Mail => "mail",
            Self::Calendar => "calendar",
            Self::Callback => "callback",
            Self::Briefcase => "briefcase",
            Self::Heart => "heart",
            Self::Dot => "dot",
        }
    }

    /// The icon saved under `id`.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|icon| icon.id() == id.trim())
    }

    /// A readable name for menus and screen readers.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Checkbox => "Check box",
            Self::Star => "Star",
            Self::Question => "Question mark",
            Self::Marker => "Marker",
            Self::Person => "Person",
            Self::Pin => "Map pin",
            Self::Phone => "Telephone",
            Self::Globe => "Globe",
            Self::Bulb => "Light bulb",
            Self::Key => "Key",
            Self::Alert => "Exclamation mark",
            Self::Flag => "Flag",
            Self::Film => "Film",
            Self::Book => "Book",
            Self::Note => "Music note",
            Self::Quote => "Quotation marks",
            Self::Pencil => "Pencil",
            Self::Chat => "Speech bubble",
            Self::Mail => "Envelope",
            Self::Calendar => "Calendar",
            Self::Callback => "Call back",
            Self::Briefcase => "Briefcase",
            Self::Heart => "Heart",
            Self::Dot => "Dot",
        }
    }

    /// The color the icon is drawn in unless the tag names its own, as 0xRRGGBB.
    pub const fn color(self) -> u32 {
        match self {
            Self::Checkbox => 0x3F7FBF,
            Self::Star => 0xF2B01E,
            Self::Question => 0x2F7BD6,
            Self::Marker => 0xE0B800,
            Self::Person => 0x5B6EAE,
            Self::Pin => 0xD9534F,
            Self::Phone => 0x2E9B5E,
            Self::Globe => 0x2A8CB8,
            Self::Bulb => 0xF2B01E,
            Self::Key => 0xA07A2C,
            Self::Alert => 0xD93025,
            Self::Flag => 0x3F7FBF,
            Self::Film => 0x6A4C93,
            Self::Book => 0x8D5A36,
            Self::Note => 0x7B52AB,
            Self::Quote => 0x607D8B,
            Self::Pencil => 0xE07B24,
            Self::Chat => 0x3F7FBF,
            Self::Mail => 0x4A6FA5,
            Self::Calendar => 0xD9534F,
            Self::Callback => 0x2E9B5E,
            Self::Briefcase => 0x7A5C3E,
            Self::Heart => 0xD6336C,
            Self::Dot => 0x607D8B,
        }
    }
}

/// How a tag looks on the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagStyle {
    /// The check box of a task list item (To Do).
    Checkbox,
    /// An icon at the start of the paragraph.
    Icon,
    /// A highlight behind the paragraph text instead of an icon.
    Highlight,
}

/// One tag of the gallery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagDef {
    /// What is stored after the `#`, lowercase.
    pub slug: String,
    /// The name shown in the gallery and the Find Tags pane.
    pub name: String,
    pub icon: TagIcon,
    /// The icon color as 0xRRGGBB.
    pub icon_color: u32,
    pub style: TagStyle,
    /// Text color for the tagged paragraph, as 0xRRGGBB.
    pub font_color: Option<u32>,
    /// Highlight color behind the tagged paragraph, as 0xRRGGBB.
    pub highlight: Option<u32>,
    /// One of OneNote's default tags: it cannot be renamed or deleted.
    pub builtin: bool,
}

/// A custom tag as kept in settings: plain values, no checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedTag {
    pub slug: String,
    pub name: String,
    pub icon: String,
    pub font_color: Option<u32>,
    pub highlight: Option<u32>,
}

/// Why a custom tag change was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagError {
    /// The name is empty or too long.
    BadName,
    /// Another tag has this name.
    DuplicateName,
    /// There are already [`MAX_CUSTOM_TAGS`] custom tags.
    TooMany,
    /// No custom tag has this slug (default tags cannot be changed).
    Unknown,
}

impl std::fmt::Display for TagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::BadName => "the tag name must be 1 to 40 characters",
            Self::DuplicateName => "another tag has this name",
            Self::TooMany => "there are too many custom tags",
            Self::Unknown => "no custom tag has this id",
        })
    }
}

/// The default tags, in gallery order: the first nine have Ctrl+1 to Ctrl+9.
/// (slug, name, icon, icon color, style, highlight color)
type Row = (
    &'static str,
    &'static str,
    TagIcon,
    u32,
    TagStyle,
    Option<u32>,
);

const ICON: TagStyle = TagStyle::Icon;
#[rustfmt::skip]
const DEFAULTS: [Row; 29] = [
    ("todo", "To Do", TagIcon::Checkbox, 0x3F7FBF, TagStyle::Checkbox, None),
    ("important", "Important", TagIcon::Star, 0xF2B01E, ICON, None),
    ("question", "Question", TagIcon::Question, 0x2F7BD6, ICON, None),
    ("remember", "Remember for later", TagIcon::Marker, 0xE59A00, TagStyle::Highlight, Some(0xFFD180)),
    ("definition", "Definition", TagIcon::Marker, 0x1BA9B8, TagStyle::Highlight, Some(0x80DEEA)),
    ("highlight", "Highlight", TagIcon::Marker, 0xE0B800, TagStyle::Highlight, Some(0xFFF176)),
    ("contact", "Contact", TagIcon::Person, 0x5B6EAE, ICON, None),
    ("address", "Address", TagIcon::Pin, 0xD9534F, ICON, None),
    ("phone", "Phone number", TagIcon::Phone, 0x2E9B5E, ICON, None),
    ("website", "Web site to visit", TagIcon::Globe, 0x2A8CB8, ICON, None),
    ("idea", "Idea", TagIcon::Bulb, 0xF2B01E, ICON, None),
    ("password", "Password", TagIcon::Key, 0xA07A2C, ICON, None),
    ("critical", "Critical", TagIcon::Alert, 0xD93025, ICON, None),
    ("project-a", "Project A", TagIcon::Flag, 0x3F7FBF, ICON, None),
    ("project-b", "Project B", TagIcon::Flag, 0x2E9B5E, ICON, None),
    ("movie", "Movie to see", TagIcon::Film, 0x6A4C93, ICON, None),
    ("book", "Book to read", TagIcon::Book, 0x8D5A36, ICON, None),
    ("music", "Music to listen to", TagIcon::Note, 0x7B52AB, ICON, None),
    ("source", "Source for article", TagIcon::Quote, 0x607D8B, ICON, None),
    ("blog", "Remember for blog", TagIcon::Pencil, 0xE07B24, ICON, None),
    ("discuss-a", "Discuss with Person A", TagIcon::Chat, 0x3F7FBF, ICON, None),
    ("discuss-b", "Discuss with Person B", TagIcon::Chat, 0x2E9B5E, ICON, None),
    ("discuss-manager", "Discuss with manager", TagIcon::Chat, 0xE07B24, ICON, None),
    ("email", "Send in email", TagIcon::Mail, 0x4A6FA5, ICON, None),
    ("meeting", "Schedule meeting", TagIcon::Calendar, 0xD9534F, ICON, None),
    ("callback", "Call back", TagIcon::Callback, 0x2E9B5E, ICON, None),
    ("todo-1", "To Do priority 1", TagIcon::Checkbox, 0xD93025, ICON, None),
    ("todo-2", "To Do priority 2", TagIcon::Checkbox, 0xE07B24, ICON, None),
    ("client", "Client request", TagIcon::Briefcase, 0x7A5C3E, ICON, None),
];

/// How many default tags there are.
pub const DEFAULT_COUNT: usize = DEFAULTS.len();

/// The default gallery, in order.
pub fn default_tags() -> Vec<TagDef> {
    DEFAULTS
        .iter()
        .map(|&(slug, name, icon, icon_color, style, highlight)| TagDef {
            slug: slug.to_owned(),
            name: name.to_owned(),
            icon,
            icon_color,
            style,
            font_color: None,
            highlight,
            builtin: true,
        })
        .collect()
}

/// The slugs of the default tags with a Ctrl+number shortcut, in key order (Ctrl+1 first).
pub fn shortcut_slugs() -> impl Iterator<Item = &'static str> {
    DEFAULTS.iter().take(9).map(|d| d.0)
}

/// All the tags: the defaults, then the custom ones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagSet {
    tags: Vec<TagDef>,
}

impl Default for TagSet {
    fn default() -> Self {
        Self {
            tags: default_tags(),
        }
    }
}

impl TagSet {
    /// The defaults plus the saved custom tags. A saved tag that is damaged (bad slug or name,
    /// a slug already in use) is dropped; an unknown icon becomes the dot.
    pub fn from_saved<'a>(saved: impl IntoIterator<Item = &'a SavedTag>) -> Self {
        let mut set = Self::default();
        for tag in saved {
            if set.custom_count() >= MAX_CUSTOM_TAGS {
                break;
            }
            let slug = tag.slug.trim().to_ascii_lowercase();
            let name = clean_name(&tag.name);
            let (Some(name), true) = (name, is_valid_name(&slug)) else {
                continue;
            };
            if set.get(&slug).is_some() || set.name_in_use(&name, None) {
                continue;
            }
            set.tags.push(custom_def(
                slug,
                name,
                TagIcon::from_id(&tag.icon).unwrap_or(TagIcon::Dot),
                tag.font_color,
                tag.highlight,
            ));
        }
        set
    }

    /// The custom tags as they are saved.
    pub fn to_saved(&self) -> Vec<SavedTag> {
        self.custom()
            .map(|t| SavedTag {
                slug: t.slug.clone(),
                name: t.name.clone(),
                icon: t.icon.id().to_owned(),
                font_color: t.font_color,
                highlight: t.highlight,
            })
            .collect()
    }

    /// Every tag, defaults first.
    pub fn all(&self) -> &[TagDef] {
        &self.tags
    }

    /// The custom tags.
    pub fn custom(&self) -> impl Iterator<Item = &TagDef> {
        self.tags.iter().filter(|t| !t.builtin)
    }

    pub fn custom_count(&self) -> usize {
        self.custom().count()
    }

    /// The tag stored as `#slug` (any case).
    pub fn get(&self, slug: &str) -> Option<&TagDef> {
        self.tags
            .iter()
            .find(|t| t.slug.eq_ignore_ascii_case(slug.trim()))
    }

    /// The tag at a Ctrl+number position (1 to 9).
    pub fn shortcut(&self, number: usize) -> Option<&TagDef> {
        number
            .checked_sub(1)
            .filter(|&n| n < 9)
            .and_then(|n| self.tags.get(n))
    }

    fn name_in_use(&self, name: &str, except: Option<&str>) -> bool {
        let key = name_key(name);
        self.tags
            .iter()
            .any(|t| name_key(&t.name) == key && Some(t.slug.as_str()) != except)
    }

    /// Add a custom tag. Its slug is made from the name (and never changes after, so a rename
    /// leaves the pages alone). Returns the slug.
    pub fn add_custom(
        &mut self,
        name: &str,
        icon: TagIcon,
        font_color: Option<u32>,
        highlight: Option<u32>,
    ) -> Result<String, TagError> {
        if self.custom_count() >= MAX_CUSTOM_TAGS {
            return Err(TagError::TooMany);
        }
        let name = clean_name(name).ok_or(TagError::BadName)?;
        if self.name_in_use(&name, None) {
            return Err(TagError::DuplicateName);
        }
        let slug = self.free_slug(&name);
        self.tags
            .push(custom_def(slug.clone(), name, icon, font_color, highlight));
        Ok(slug)
    }

    /// Change a custom tag. `None` leaves a field alone; `Some(None)` clears a color.
    pub fn edit_custom(
        &mut self,
        slug: &str,
        name: Option<&str>,
        icon: Option<TagIcon>,
        font_color: Option<Option<u32>>,
        highlight: Option<Option<u32>>,
    ) -> Result<(), TagError> {
        let Some(index) = self
            .tags
            .iter()
            .position(|t| !t.builtin && t.slug.eq_ignore_ascii_case(slug.trim()))
        else {
            return Err(TagError::Unknown);
        };
        let new_name = match name {
            Some(name) => {
                let name = clean_name(name).ok_or(TagError::BadName)?;
                if self.name_in_use(
                    &name,
                    Some(
                        &self
                            .tags
                            .get(index)
                            .map_or_else(String::new, |t| t.slug.clone()),
                    ),
                ) {
                    return Err(TagError::DuplicateName);
                }
                Some(name)
            }
            None => None,
        };
        let Some(tag) = self.tags.get_mut(index) else {
            return Err(TagError::Unknown);
        };
        if let Some(name) = new_name {
            tag.name = name;
        }
        if let Some(icon) = icon {
            tag.icon = icon;
            tag.icon_color = icon.color();
        }
        if let Some(color) = font_color {
            tag.font_color = color;
        }
        if let Some(color) = highlight {
            tag.highlight = color;
        }
        Ok(())
    }

    /// Delete a custom tag. Text in pages that says `#slug` stays; it is then an ordinary
    /// word-like tag without a gallery entry.
    pub fn delete_custom(&mut self, slug: &str) -> Result<(), TagError> {
        let before = self.tags.len();
        self.tags
            .retain(|t| t.builtin || !t.slug.eq_ignore_ascii_case(slug.trim()));
        if self.tags.len() == before {
            Err(TagError::Unknown)
        } else {
            Ok(())
        }
    }

    /// A slug for `name` that no tag uses.
    fn free_slug(&self, name: &str) -> String {
        let base = slug_of(name);
        if self.get(&base).is_none() {
            return base;
        }
        for n in 2..=1000 {
            let suffix = format!("-{n}");
            let mut stem = base.clone();
            stem.truncate(MAX_SLUG_BYTES - suffix.len());
            let candidate = format!("{stem}{suffix}");
            if self.get(&candidate).is_none() {
                return candidate;
            }
        }
        format!("tag-{}", self.tags.len())
    }
}

fn custom_def(
    slug: String,
    name: String,
    icon: TagIcon,
    font_color: Option<u32>,
    highlight: Option<u32>,
) -> TagDef {
    TagDef {
        slug,
        name,
        icon,
        icon_color: icon.color(),
        style: TagStyle::Icon,
        font_color,
        highlight,
        builtin: false,
    }
}

/// What two names are compared by: letters and digits, ignoring case, spacing, and
/// punctuation (the whole name when it has none).
fn name_key(name: &str) -> String {
    let key: String = name
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    if key.is_empty() {
        name.trim().to_lowercase()
    } else {
        key
    }
}

/// The name trimmed and checked: 1 to 40 characters, no control characters.
fn clean_name(name: &str) -> Option<String> {
    let name = name.trim();
    let count = name.chars().count();
    ((1..=MAX_TAG_NAME_CHARS).contains(&count) && !name.chars().any(char::is_control))
        .then(|| name.to_owned())
}

/// The slug for a display name: lowercase ASCII letters and digits with single hyphens
/// between words, starting with a letter. A name with no ASCII letters becomes `tag`.
fn slug_of(name: &str) -> String {
    let mut slug = String::new();
    let mut gap = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            if gap && !slug.is_empty() {
                slug.push('-');
            }
            gap = false;
            slug.push(c.to_ascii_lowercase());
        } else {
            gap = true;
        }
    }
    let slug = slug.trim_start_matches(|c: char| !c.is_ascii_alphabetic());
    let mut slug: String = slug.chars().take(MAX_SLUG_BYTES).collect();
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "tag".to_owned()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_have_unique_valid_slugs_and_names() {
        let tags = default_tags();
        assert_eq!(tags.len(), 29);
        for tag in &tags {
            assert!(is_valid_name(&tag.slug), "{}", tag.slug);
            assert_eq!(tag.slug, tag.slug.to_ascii_lowercase());
            assert!(tag.builtin);
        }
        let mut slugs: Vec<&str> = tags.iter().map(|t| t.slug.as_str()).collect();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), 29);
        let mut names: Vec<String> = tags.iter().map(|t| t.name.to_lowercase()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 29);
    }

    #[test]
    fn the_first_nine_follow_onenote_shortcuts() {
        let set = TagSet::default();
        let first: Vec<&str> = (1..=9)
            .filter_map(|n| set.shortcut(n))
            .map(|t| t.slug.as_str())
            .collect();
        assert_eq!(
            first,
            [
                "todo",
                "important",
                "question",
                "remember",
                "definition",
                "highlight",
                "contact",
                "address",
                "phone"
            ]
        );
        assert!(set.shortcut(0).is_none() && set.shortcut(10).is_none());
        assert_eq!(set.get("IDEA").map(|t| t.name.as_str()), Some("Idea"));
    }

    #[test]
    fn highlight_tags_have_a_highlight_color() {
        for tag in default_tags() {
            assert_eq!(
                tag.style == TagStyle::Highlight,
                tag.highlight.is_some(),
                "{}",
                tag.slug
            );
        }
    }

    #[test]
    fn custom_tags_get_unique_slugs_from_their_names() {
        let mut set = TagSet::default();
        assert_eq!(
            set.add_custom("Read Later!", TagIcon::Book, None, None),
            Ok("read-later".to_owned())
        );
        assert_eq!(
            set.add_custom("read  later?", TagIcon::Book, None, None),
            Err(TagError::DuplicateName)
        );
        assert_eq!(
            set.add_custom("Idea", TagIcon::Bulb, None, None),
            Err(TagError::DuplicateName),
            "a default name"
        );
        // A name that makes the slug of a default tag gets a suffix.
        assert_eq!(
            set.add_custom("Client", TagIcon::Dot, None, None),
            Ok("client-2".to_owned())
        );
        assert_eq!(
            set.add_custom("123", TagIcon::Dot, None, None),
            Ok("tag".to_owned())
        );
        assert_eq!(
            set.add_custom("\u{4e2d}\u{6587}", TagIcon::Dot, None, None),
            Ok("tag-2".to_owned())
        );
        assert_eq!(
            set.add_custom("   ", TagIcon::Dot, None, None),
            Err(TagError::BadName)
        );
        assert_eq!(
            set.add_custom(&"x".repeat(41), TagIcon::Dot, None, None),
            Err(TagError::BadName)
        );
        for tag in set.custom() {
            assert!(is_valid_name(&tag.slug), "{}", tag.slug);
        }
    }

    #[test]
    fn rename_keeps_the_slug_and_defaults_cannot_change() {
        let mut set = TagSet::default();
        let slug = set
            .add_custom("Errand", TagIcon::Pin, Some(0xFF0000), None)
            .unwrap();
        set.edit_custom(
            &slug,
            Some("Chore"),
            Some(TagIcon::Star),
            Some(None),
            Some(Some(0xFFF176)),
        )
        .unwrap();
        let tag = set.get(&slug).unwrap();
        assert_eq!((tag.name.as_str(), tag.icon), ("Chore", TagIcon::Star));
        assert_eq!((tag.font_color, tag.highlight), (None, Some(0xFFF176)));
        assert_eq!(
            set.edit_custom("idea", Some("Thought"), None, None, None),
            Err(TagError::Unknown)
        );
        assert_eq!(
            set.edit_custom(&slug, Some("Idea"), None, None, None),
            Err(TagError::DuplicateName)
        );
        assert_eq!(set.delete_custom("idea"), Err(TagError::Unknown));
        assert_eq!(set.delete_custom(&slug), Ok(()));
        assert!(set.get(&slug).is_none());
    }

    #[test]
    fn saved_tags_round_trip_and_damaged_ones_are_dropped() {
        let mut set = TagSet::default();
        set.add_custom("Errand", TagIcon::Pin, Some(0x112233), Some(0x445566))
            .unwrap();
        let saved = set.to_saved();
        assert_eq!(TagSet::from_saved(&saved), set);
        let bad = [
            SavedTag {
                slug: "ok".into(),
                name: "Fine".into(),
                icon: "no-such-icon".into(),
                font_color: None,
                highlight: None,
            },
            SavedTag {
                slug: "idea".into(),
                name: "Clash".into(),
                icon: "dot".into(),
                font_color: None,
                highlight: None,
            },
            SavedTag {
                slug: "9x".into(),
                name: "Bad slug".into(),
                icon: "dot".into(),
                font_color: None,
                highlight: None,
            },
            SavedTag {
                slug: "empty".into(),
                name: "  ".into(),
                icon: "dot".into(),
                font_color: None,
                highlight: None,
            },
        ];
        let set = TagSet::from_saved(&bad);
        assert_eq!(set.custom_count(), 1);
        assert_eq!(set.get("ok").map(|t| t.icon), Some(TagIcon::Dot));
    }

    #[test]
    fn the_custom_count_is_capped() {
        let mut set = TagSet::default();
        for n in 0..MAX_CUSTOM_TAGS {
            set.add_custom(&format!("Tag {n}"), TagIcon::Dot, None, None)
                .unwrap();
        }
        assert_eq!(
            set.add_custom("One more", TagIcon::Dot, None, None),
            Err(TagError::TooMany)
        );
    }

    #[test]
    fn icon_ids_round_trip() {
        for icon in TagIcon::ALL {
            assert_eq!(TagIcon::from_id(icon.id()), Some(icon));
        }
        assert_eq!(TagIcon::from_id("nope"), None);
    }
}
