//! The note tag gallery: OneNote's default tags with the slugs they are stored under, and the
//! custom tags a person adds in the Customize Tags dialog (docs/markdown.md section 4).
//!
//! This is data only. The canvas draws each [`TagIcon`] in code; settings keep the custom
//! tags and the changes to the default ones (the app saves a [`SavedTags`] value). A default
//! tag keeps its slug whatever is changed, so the text in pages never changes.

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
    /// One of OneNote's default tags: its slug is fixed. It can be restyled and hidden from the
    /// gallery, but not deleted.
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

/// What a person changed about a default tag, as kept in settings. A field is `None` when the
/// tag still has its built-in value, so a later change to a default still applies. The inner
/// `Option` of a color is the color itself, `None` meaning no color.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SavedOverride {
    pub slug: String,
    pub name: Option<String>,
    pub icon: Option<String>,
    pub font_color: Option<Option<u32>>,
    pub highlight: Option<Option<u32>>,
}

/// Everything about the gallery that settings keep: plain values, no checks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SavedTags {
    pub custom: Vec<SavedTag>,
    /// Changes to default tags.
    pub overrides: Vec<SavedOverride>,
    /// Slugs of the tags shown, in order. Empty means the built-in order (the defaults, then the
    /// custom tags).
    pub order: Vec<String>,
    /// Slugs of the default tags taken out of the gallery.
    pub hidden: Vec<String>,
}

/// Which way a tag moves in the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveDir {
    Up,
    Down,
}

/// Why a tag change was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagError {
    /// The name is empty or too long.
    BadName,
    /// Another tag has this name.
    DuplicateName,
    /// There are already [`MAX_CUSTOM_TAGS`] custom tags.
    TooMany,
    /// No tag of the right kind has this slug.
    Unknown,
}

impl std::fmt::Display for TagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::BadName => "the tag name must be 1 to 40 characters",
            Self::DuplicateName => "another tag has this name",
            Self::TooMany => "there are too many custom tags",
            Self::Unknown => "no tag has this id",
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

/// The built-in look of the default tag `slug`.
fn default_of(slug: &str) -> Option<TagDef> {
    default_tags()
        .into_iter()
        .find(|t| t.slug.eq_ignore_ascii_case(slug.trim()))
}

fn default_slugs() -> impl Iterator<Item = &'static str> + Clone {
    DEFAULTS.iter().map(|d| d.0)
}

/// The tags: the ones in the gallery, in the order a person set (the first nine have
/// Ctrl+1 to Ctrl+9), and the default tags taken out of the gallery. A hidden tag is still
/// known, so text in pages that says `#slug` keeps its icon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagSet {
    tags: Vec<TagDef>,
    hidden: Vec<TagDef>,
}

impl Default for TagSet {
    fn default() -> Self {
        Self {
            tags: default_tags(),
            hidden: Vec::new(),
        }
    }
}

impl TagSet {
    /// The defaults plus the saved custom tags. A saved tag that is damaged (bad slug or name,
    /// a slug already in use) is dropped; an unknown icon becomes the dot.
    pub fn from_saved<'a>(saved: impl IntoIterator<Item = &'a SavedTag>) -> Self {
        let mut set = Self::default();
        set.add_saved_custom(saved);
        set
    }

    /// The gallery as settings keep it: changes to default tags, the custom tags, which
    /// defaults are hidden, and the order. Every damaged entry is dropped on its own (a bad
    /// field of a change is ignored, the rest of it applies).
    pub fn restore(saved: &SavedTags) -> Self {
        let mut set = Self::default();
        for change in &saved.overrides {
            set.apply_override(change);
        }
        set.add_saved_custom(&saved.custom);
        for slug in &saved.hidden {
            let slug = slug.trim().to_ascii_lowercase();
            if let Some(i) = set.tags.iter().position(|t| t.builtin && t.slug == slug) {
                let def = set.tags.remove(i);
                set.hidden.push(def);
            }
        }
        let mut ordered: Vec<TagDef> = Vec::with_capacity(set.tags.len());
        for slug in &saved.order {
            let slug = slug.trim().to_ascii_lowercase();
            if let Some(i) = set.tags.iter().position(|t| t.slug == slug) {
                ordered.push(set.tags.remove(i));
            }
        }
        ordered.append(&mut set.tags);
        set.tags = ordered;
        set
    }

    fn add_saved_custom<'a>(&mut self, saved: impl IntoIterator<Item = &'a SavedTag>) {
        for tag in saved {
            if self.custom_count() >= MAX_CUSTOM_TAGS {
                break;
            }
            let slug = tag.slug.trim().to_ascii_lowercase();
            let name = clean_name(&tag.name);
            let (Some(name), true) = (name, is_valid_name(&slug)) else {
                continue;
            };
            if self.get(&slug).is_some() || self.name_in_use(&name, None) {
                continue;
            }
            self.tags.push(custom_def(
                slug,
                name,
                TagIcon::from_id(&tag.icon).unwrap_or(TagIcon::Dot),
                tag.font_color,
                tag.highlight,
            ));
        }
    }

    fn apply_override(&mut self, change: &SavedOverride) {
        let slug = change.slug.trim().to_ascii_lowercase();
        if !self.tags.iter().any(|t| t.builtin && t.slug == slug) {
            return;
        }
        // Each field on its own: a refused name leaves the other fields to apply.
        let name = change.name.as_deref().and_then(clean_name);
        let name = name.filter(|n| !self.name_in_use(n, Some(&slug)));
        let icon = change.icon.as_deref().and_then(TagIcon::from_id);
        let _ = self.edit(
            &slug,
            name.as_deref(),
            icon,
            change.font_color,
            change.highlight,
        );
    }

    /// Everything settings keep: the custom tags, the changes to default tags (only the
    /// fields that differ from the built-in value), the hidden defaults, and the order (only
    /// when it is not the built-in one).
    pub fn to_saved_all(&self) -> SavedTags {
        let mut overrides = Vec::new();
        for def in self.tags.iter().chain(&self.hidden).filter(|t| t.builtin) {
            let Some(base) = default_of(&def.slug) else {
                continue;
            };
            let change = SavedOverride {
                slug: def.slug.clone(),
                name: (def.name != base.name).then(|| def.name.clone()),
                icon: (def.icon != base.icon).then(|| def.icon.id().to_owned()),
                font_color: (def.font_color != base.font_color).then_some(def.font_color),
                highlight: (def.highlight != base.highlight).then_some(def.highlight),
            };
            let none = SavedOverride {
                slug: def.slug.clone(),
                ..SavedOverride::default()
            };
            if change != none {
                overrides.push(change);
            }
        }
        let builtin_order: Vec<&str> = self
            .tags
            .iter()
            .filter(|t| t.builtin)
            .map(|t| t.slug.as_str())
            .collect::<Vec<_>>();
        let mut builtin_order = {
            let mut v = builtin_order;
            v.sort_by_key(|slug| default_slugs().position(|s| s == *slug));
            v
        };
        builtin_order.extend(self.custom().map(|t| t.slug.as_str()));
        let shown: Vec<&str> = self.tags.iter().map(|t| t.slug.as_str()).collect();
        let order = if shown == builtin_order {
            Vec::new()
        } else {
            shown.iter().map(|s| (*s).to_owned()).collect()
        };
        let mut hidden: Vec<String> = self.hidden.iter().map(|t| t.slug.clone()).collect();
        hidden.sort_by_key(|slug| default_slugs().position(|s| s == slug));
        SavedTags {
            custom: self.to_saved(),
            overrides,
            order,
            hidden,
        }
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

    /// The tags in the gallery, in order.
    pub fn all(&self) -> &[TagDef] {
        &self.tags
    }

    /// The default tags taken out of the gallery.
    pub fn hidden(&self) -> &[TagDef] {
        &self.hidden
    }

    /// The custom tags.
    pub fn custom(&self) -> impl Iterator<Item = &TagDef> {
        self.tags.iter().filter(|t| !t.builtin)
    }

    pub fn custom_count(&self) -> usize {
        self.custom().count()
    }

    /// The tag stored as `#slug` (any case), hidden ones included.
    pub fn get(&self, slug: &str) -> Option<&TagDef> {
        self.tags
            .iter()
            .chain(&self.hidden)
            .find(|t| t.slug.eq_ignore_ascii_case(slug.trim()))
    }

    /// The tag at a Ctrl+number position (1 to 9): the nth tag of the gallery.
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
            .chain(&self.hidden)
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
        if self.get(slug).is_none_or(|t| t.builtin) {
            return Err(TagError::Unknown);
        }
        self.edit(slug, name, icon, font_color, highlight)
    }

    /// Change any tag, a default one included: its name, icon, and colors (the slug never
    /// changes). `None` leaves a field alone; `Some(None)` clears a color. A new icon brings
    /// its own color; going back to a default tag's own icon brings back its built-in color.
    pub fn edit(
        &mut self,
        slug: &str,
        name: Option<&str>,
        icon: Option<TagIcon>,
        font_color: Option<Option<u32>>,
        highlight: Option<Option<u32>>,
    ) -> Result<(), TagError> {
        let Some(found) = self.get(slug) else {
            return Err(TagError::Unknown);
        };
        let key = found.slug.clone();
        let new_name = match name {
            Some(name) => {
                let name = clean_name(name).ok_or(TagError::BadName)?;
                if self.name_in_use(&name, Some(&key)) {
                    return Err(TagError::DuplicateName);
                }
                Some(name)
            }
            None => None,
        };
        let Some(tag) = self
            .tags
            .iter_mut()
            .chain(self.hidden.iter_mut())
            .find(|t| t.slug == key)
        else {
            return Err(TagError::Unknown);
        };
        if let Some(name) = new_name {
            tag.name = name;
        }
        if let Some(icon) = icon {
            tag.icon = icon;
            tag.icon_color = match default_of(&key) {
                Some(base) if base.icon == icon => base.icon_color,
                _ => icon.color(),
            };
        }
        if let Some(color) = font_color {
            tag.font_color = color;
        }
        if let Some(color) = highlight {
            tag.highlight = color;
        }
        Ok(())
    }

    /// Take a default tag out of the gallery and the Ctrl+number list. Text in pages that says
    /// `#slug` keeps its icon, as in OneNote. Only [`TagSet::reset_all`] brings it back.
    pub fn hide(&mut self, slug: &str) -> Result<(), TagError> {
        let Some(i) = self
            .tags
            .iter()
            .position(|t| t.builtin && t.slug.eq_ignore_ascii_case(slug.trim()))
        else {
            return Err(TagError::Unknown);
        };
        let def = self.tags.remove(i);
        self.hidden.push(def);
        Ok(())
    }

    /// Move a tag one place up or down the list. At the end of the list nothing changes.
    pub fn move_tag(&mut self, slug: &str, dir: MoveDir) -> Result<(), TagError> {
        let Some(i) = self
            .tags
            .iter()
            .position(|t| t.slug.eq_ignore_ascii_case(slug.trim()))
        else {
            return Err(TagError::Unknown);
        };
        let j = match dir {
            MoveDir::Up => i.checked_sub(1),
            MoveDir::Down => Some(i + 1).filter(|&j| j < self.tags.len()),
        };
        if let Some(j) = j {
            self.tags.swap(i, j);
        }
        Ok(())
    }

    /// Give a default tag its built-in name, icon, and colors back (it keeps its place).
    pub fn reset_tag(&mut self, slug: &str) -> Result<(), TagError> {
        let Some(base) = default_of(slug) else {
            return Err(TagError::Unknown);
        };
        if self.get(&base.slug).is_none() {
            return Err(TagError::Unknown);
        }
        if self.name_in_use(&base.name, Some(&base.slug)) {
            return Err(TagError::DuplicateName);
        }
        let Some(tag) = self
            .tags
            .iter_mut()
            .chain(self.hidden.iter_mut())
            .find(|t| t.slug == base.slug)
        else {
            return Err(TagError::Unknown);
        };
        *tag = base;
        Ok(())
    }

    /// Whether a default tag differs from its built-in look.
    pub fn is_modified(&self, slug: &str) -> bool {
        match (self.get(slug), default_of(slug)) {
            (Some(now), Some(base)) => *now != base,
            _ => false,
        }
    }

    /// Put the built-in list back: every default tag is in the gallery in the built-in order
    /// with its built-in look. Custom tags stay, after the defaults (one whose name a default
    /// now has gets a number added to its name).
    pub fn reset_all(&mut self) {
        let custom: Vec<TagDef> = self.tags.drain(..).filter(|t| !t.builtin).collect();
        self.hidden.clear();
        self.tags = default_tags();
        for mut tag in custom {
            let base = tag.name.clone();
            let mut n = 2;
            while self.name_in_use(&tag.name, None) && n < 1000 {
                let suffix = format!(" {n}");
                let keep = MAX_TAG_NAME_CHARS.saturating_sub(suffix.chars().count());
                tag.name = format!("{}{suffix}", base.chars().take(keep).collect::<String>());
                n += 1;
            }
            self.tags.push(tag);
        }
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

    #[test]
    fn a_default_tag_keeps_its_slug_and_stores_only_what_differs() {
        let mut set = TagSet::default();
        assert_eq!(set.to_saved_all(), SavedTags::default());
        set.edit(
            "idea",
            Some("Thought"),
            Some(TagIcon::Heart),
            None,
            Some(Some(0x112233)),
        )
        .unwrap();
        // Same icon again gives the built-in icon color back (a flag keeps its green).
        set.edit("project-b", None, Some(TagIcon::Star), None, None)
            .unwrap();
        set.edit("project-b", None, Some(TagIcon::Flag), None, None)
            .unwrap();
        assert_eq!(set.get("project-b").map(|t| t.icon_color), Some(0x2E9B5E));
        let saved = set.to_saved_all();
        assert_eq!(saved.overrides.len(), 1, "{:?}", saved.overrides);
        let change = &saved.overrides[0];
        assert_eq!(change.slug, "idea");
        assert_eq!(change.name.as_deref(), Some("Thought"));
        assert_eq!(change.icon.as_deref(), Some("heart"));
        assert_eq!(change.font_color, None);
        assert_eq!(change.highlight, Some(Some(0x112233)));
        assert!(saved.order.is_empty() && saved.hidden.is_empty());
        assert_eq!(TagSet::restore(&saved), set);
        assert!(set.is_modified("idea") && !set.is_modified("question"));
        // A name another tag has is refused; so is a bad one.
        assert_eq!(
            set.edit("idea", Some("question"), None, None, None),
            Err(TagError::DuplicateName)
        );
        assert_eq!(
            set.edit("idea", Some(" "), None, None, None),
            Err(TagError::BadName)
        );
        assert_eq!(
            set.edit("nope", None, None, None, None),
            Err(TagError::Unknown)
        );
    }

    #[test]
    fn hidden_and_ordered_tags_round_trip_and_hidden_ones_stay_known() {
        let mut set = TagSet::default();
        set.hide("todo").unwrap();
        set.hide("idea").unwrap();
        set.edit("idea", Some("Thought"), None, None, None).unwrap();
        for _ in 0..4 {
            set.move_tag("question", MoveDir::Down).unwrap();
        }
        let custom = set.add_custom("Errand", TagIcon::Pin, None, None).unwrap();
        set.move_tag(&custom, MoveDir::Up).unwrap();
        assert_eq!(set.get("todo").map(|t| t.name.as_str()), Some("To Do"));
        assert_eq!(set.hide("errand"), Err(TagError::Unknown));
        assert_eq!(set.shortcut(1).map(|t| t.slug.as_str()), Some("important"));
        let saved = set.to_saved_all();
        assert_eq!(saved.hidden, ["todo", "idea"]);
        assert!(!saved.order.is_empty());
        let back = TagSet::restore(&saved);
        assert_eq!(back, set);
        let shown: Vec<&str> = back.all().iter().map(|t| t.slug.as_str()).collect();
        let was: Vec<&str> = set.all().iter().map(|t| t.slug.as_str()).collect();
        assert_eq!(shown, was);
        // The hidden tag name still counts, so no second "Thought".
        let mut other = back.clone();
        assert_eq!(
            other.add_custom("Thought", TagIcon::Dot, None, None),
            Err(TagError::DuplicateName)
        );
        // Reset puts everything back; custom tags stay, after the defaults.
        other.reset_all();
        assert!(other.hidden().is_empty());
        assert_eq!(other.all().len(), 30);
        assert_eq!(other.all().first().map(|t| t.slug.as_str()), Some("todo"));
        assert_eq!(other.all().last().map(|t| t.slug.as_str()), Some("errand"));
        assert_eq!(other.get("idea").map(|t| t.name.as_str()), Some("Idea"));
        assert_eq!(other.to_saved_all().overrides, Vec::new());
    }

    #[test]
    fn moving_stops_at_the_ends_and_shortcuts_follow_the_list() {
        let mut set = TagSet::default();
        set.move_tag("todo", MoveDir::Up).unwrap();
        assert_eq!(set.shortcut(1).map(|t| t.slug.as_str()), Some("todo"));
        set.move_tag("client", MoveDir::Down).unwrap();
        assert_eq!(set.all().last().map(|t| t.slug.as_str()), Some("client"));
        set.move_tag("phone", MoveDir::Down).unwrap();
        assert_eq!(set.shortcut(9).map(|t| t.slug.as_str()), Some("website"));
        assert_eq!(set.move_tag("nope", MoveDir::Up), Err(TagError::Unknown));
    }

    #[test]
    fn damaged_saved_changes_are_dropped_one_by_one() {
        let saved = SavedTags {
            custom: vec![SavedTag {
                slug: "errand".into(),
                name: "Errand".into(),
                icon: "pin".into(),
                font_color: None,
                highlight: None,
            }],
            overrides: vec![
                SavedOverride {
                    slug: "question".into(),
                    name: Some("  ".into()),
                    icon: Some("no-such-icon".into()),
                    font_color: Some(Some(0xFF0000)),
                    highlight: None,
                },
                SavedOverride {
                    slug: "idea".into(),
                    name: Some("Question".into()),
                    ..SavedOverride::default()
                },
                SavedOverride {
                    slug: "errand".into(),
                    name: Some("Not a default".into()),
                    ..SavedOverride::default()
                },
            ],
            order: vec![
                "errand".into(),
                "errand".into(),
                "gone".into(),
                "todo".into(),
            ],
            hidden: vec!["important".into(), "errand".into(), "gone".into()],
        };
        let set = TagSet::restore(&saved);
        assert_eq!(
            set.get("question").and_then(|t| t.font_color),
            Some(0xFF0000)
        );
        assert_eq!(
            set.get("question").map(|t| (t.name.as_str(), t.icon)),
            Some(("Question", TagIcon::Question))
        );
        assert_eq!(set.get("idea").map(|t| t.name.as_str()), Some("Idea"));
        assert_eq!(set.get("errand").map(|t| t.name.as_str()), Some("Errand"));
        assert_eq!(set.hidden().len(), 1);
        let first: Vec<&str> = set.all().iter().take(3).map(|t| t.slug.as_str()).collect();
        assert_eq!(first, ["errand", "todo", "question"]);
        assert_eq!(set.all().len(), 29);
    }
}
