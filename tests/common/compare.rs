//! Structural comparison of a brep-to-step file against step-io's.
//!
//! The two writers number entities in different orders, so the comparison
//! ignores `#ids`: every DATA entity gets a fingerprint — its type, its
//! attribute values, and in place of each reference the fingerprint of the
//! entity referred to — and the two files must hold the same multiset of
//! fingerprints. The HEADER is not compared, except for `FILE_SCHEMA`.
//!
//! Strings are compared decoded: brep-to-step writes non-ASCII characters
//! as `\X2\`/`\X4\` escapes and `\` as `\\`, and step-io's parser undoes
//! every escape on reading.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};

use step_io::parser::{Attribute, RawEntity, parse};

/// Compare a brep-to-step file (`ours`) with a step-io file (`step_io`).
///
/// Every check runs, and every problem found is reported in the one `Err`,
/// so one difference never hides another.
pub fn compare(ours: &str, step_io: &str) -> Result<(), String> {
    let ours =
        parse(ours.as_bytes()).map_err(|e| format!("brep-to-step output does not parse: {e}"))?;
    let theirs =
        parse(step_io.as_bytes()).map_err(|e| format!("step-io output does not parse: {e}"))?;
    let mut problems = Vec::new();

    let ours_schema = header_attributes(&ours.header, "FILE_SCHEMA");
    let theirs_schema = header_attributes(&theirs.header, "FILE_SCHEMA");
    if ours_schema != theirs_schema {
        problems.push(format!(
            "FILE_SCHEMA differs: brep-to-step {ours_schema:?}, step-io {theirs_schema:?}"
        ));
    }

    match (Census::of(&ours.entities), Census::of(&theirs.entities)) {
        (Ok(ours), Ok(theirs)) => problems.extend(ours.differences(&theirs)),
        (ours, theirs) => {
            if let Err(e) = ours {
                problems.push(format!("brep-to-step output: {e}"));
            }
            if let Err(e) = theirs {
                problems.push(format!("step-io output: {e}"));
            }
        }
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

/// [`compare`], panicking with the report on a difference.
pub fn assert_same_structure(ours: &str, step_io: &str) {
    if let Err(report) = compare(ours, step_io) {
        panic!("brep-to-step and step-io files differ:\n{report}");
    }
}

/// The attributes of the HEADER entity `name`, if present.
pub fn header_attributes(header: &[RawEntity], name: &str) -> Option<Vec<Attribute>> {
    header.iter().find_map(|e| match e {
        RawEntity::Simple {
            name: n,
            attributes,
            ..
        } if n == name => Some(attributes.clone()),
        _ => None,
    })
}

/// Every entity's fingerprint, counted, with a readable form of each.
struct Census {
    counts: HashMap<u64, (usize, String)>,
}

impl Census {
    fn of(entities: &BTreeMap<u64, RawEntity>) -> Result<Self, String> {
        let mut prints = Fingerprints {
            entities,
            memo: HashMap::new(),
            on_path: HashSet::new(),
        };
        let mut counts: HashMap<u64, (usize, String)> = HashMap::new();
        for (&id, entity) in entities {
            let print = prints.of(id)?;
            counts
                .entry(print)
                .or_insert_with(|| (0, describe(entity, entities)))
                .0 += 1;
        }
        Ok(Self { counts })
    }

    /// One line per fingerprint whose count differs.
    fn differences(&self, theirs: &Self) -> Vec<String> {
        let mut lines = Vec::new();
        for (print, (n, text)) in &self.counts {
            let m = theirs.counts.get(print).map_or(0, |c| c.0);
            if *n != m {
                lines.push(format!("brep-to-step {n}x, step-io {m}x: {text}"));
            }
        }
        for (print, (m, text)) in &theirs.counts {
            if !self.counts.contains_key(print) {
                lines.push(format!("brep-to-step 0x, step-io {m}x: {text}"));
            }
        }
        lines.sort();
        lines
    }
}

/// Memoized fingerprints over one file's entities.
struct Fingerprints<'a> {
    entities: &'a BTreeMap<u64, RawEntity>,
    memo: HashMap<u64, u64>,
    on_path: HashSet<u64>,
}

impl Fingerprints<'_> {
    fn of(&mut self, id: u64) -> Result<u64, String> {
        if let Some(&print) = self.memo.get(&id) {
            return Ok(print);
        }
        if !self.on_path.insert(id) {
            return Err(format!("reference cycle through #{id}"));
        }
        let entities = self.entities;
        let entity = entities
            .get(&id)
            .ok_or_else(|| format!("reference to missing #{id}"))?;
        let mut h = DefaultHasher::new();
        match entity {
            RawEntity::Simple {
                name, attributes, ..
            } => {
                0u8.hash(&mut h);
                name.hash(&mut h);
                self.hash_attributes(attributes, &mut h)?;
            }
            RawEntity::Complex { parts, .. } => {
                1u8.hash(&mut h);
                parts.len().hash(&mut h);
                for part in parts {
                    part.name.hash(&mut h);
                    self.hash_attributes(&part.attributes, &mut h)?;
                }
            }
        }
        let print = h.finish();
        self.on_path.remove(&id);
        self.memo.insert(id, print);
        Ok(print)
    }

    fn hash_attributes(
        &mut self,
        attributes: &[Attribute],
        h: &mut DefaultHasher,
    ) -> Result<(), String> {
        attributes.len().hash(h);
        for a in attributes {
            self.hash_attribute(a, h)?;
        }
        Ok(())
    }

    fn hash_attribute(&mut self, a: &Attribute, h: &mut DefaultHasher) -> Result<(), String> {
        match a {
            Attribute::Integer(i) => (0u8, i).hash(h),
            // `+ 0.0` folds -0 into +0: the same point.
            Attribute::Real(r) => (1u8, (r + 0.0).to_bits()).hash(h),
            Attribute::String(s) => (2u8, s).hash(h),
            Attribute::Enum(s) => (3u8, s).hash(h),
            Attribute::Binary(s) => (4u8, s).hash(h),
            Attribute::EntityRef(id) => (5u8, self.of(*id)?).hash(h),
            Attribute::Unset => 6u8.hash(h),
            Attribute::Derived => 7u8.hash(h),
            Attribute::List(items) => {
                8u8.hash(h);
                self.hash_attributes(items, h)?;
            }
            Attribute::Typed { type_name, value } => {
                (9u8, type_name).hash(h);
                self.hash_attribute(value, h)?;
            }
        }
        Ok(())
    }
}

/// `NAME(attrs)` with each reference shown as the type it points to.
fn describe(entity: &RawEntity, entities: &BTreeMap<u64, RawEntity>) -> String {
    match entity {
        RawEntity::Simple {
            name, attributes, ..
        } => format!("{name}({})", describe_list(attributes, entities)),
        RawEntity::Complex { parts, .. } => {
            let parts: Vec<String> = parts
                .iter()
                .map(|p| format!("{}({})", p.name, describe_list(&p.attributes, entities)))
                .collect();
            format!("({})", parts.join(" "))
        }
    }
}

fn describe_list(attributes: &[Attribute], entities: &BTreeMap<u64, RawEntity>) -> String {
    attributes
        .iter()
        .map(|a| describe_attribute(a, entities))
        .collect::<Vec<_>>()
        .join(",")
}

fn describe_attribute(a: &Attribute, entities: &BTreeMap<u64, RawEntity>) -> String {
    match a {
        Attribute::Integer(i) => i.to_string(),
        Attribute::Real(r) => format!("{r:?}"),
        Attribute::String(s) => format!("{s:?}"),
        Attribute::Enum(s) => format!(".{s}."),
        Attribute::Binary(s) => format!("\"{s}\""),
        Attribute::EntityRef(id) => match entities.get(id) {
            Some(RawEntity::Simple { name, .. }) => format!("#{name}"),
            Some(RawEntity::Complex { parts, .. }) => {
                let names: Vec<&str> = parts.iter().map(|p| p.name.as_str()).collect();
                format!("#({})", names.join(" "))
            }
            None => format!("#{id}?"),
        },
        Attribute::Unset => "$".to_owned(),
        Attribute::Derived => "*".to_owned(),
        Attribute::List(items) => format!("({})", describe_list(items, entities)),
        Attribute::Typed { type_name, value } => {
            format!("{type_name}({})", describe_attribute(value, entities))
        }
    }
}
