//! mpv's nodes and JSON, both ways: a property's value as the engine reads it, and a node built
//! from the JSON the engine sends, for `mpv_set_property` and `mpv_command_node`.
//!
//! The engine speaks of a node as JSON text (`MpvValue::Node`, `PlayerReport::Property`), as
//! `mpv_observe_property` with `MPV_FORMAT_NODE` gives it, so these are the only two directions
//! there are.

#![allow(unsafe_code)]
// Without libmpv only the tests read nodes.
#![cfg_attr(not(feature = "libmpv"), allow(dead_code))]

use std::ffi::{CStr, CString, c_char};

use serde_json::{Map, Number, Value};

use crate::sys::{
    MPV_FORMAT_BYTE_ARRAY, MPV_FORMAT_DOUBLE, MPV_FORMAT_FLAG, MPV_FORMAT_INT64,
    MPV_FORMAT_NODE_ARRAY, MPV_FORMAT_NODE_MAP, MPV_FORMAT_NONE, MPV_FORMAT_STRING, mpv_node,
    mpv_node_list, mpv_node_u,
};

/// A node mpv reads and Rust owns: the strings, lists and keys it points into live as long as
/// it, so it can be handed to any call that takes an `mpv_node *` and only reads it.
pub struct OwnedNode {
    node: mpv_node,
    /// What the node points into, kept alive and never moved: each is boxed or heap-allocated.
    _strings: Vec<CString>,
    // Boxed so that a list's address holds while the Vec grows: nodes point at it.
    #[allow(clippy::vec_box)]
    _lists: Vec<Box<mpv_node_list>>,
    _values: Vec<Vec<mpv_node>>,
    _keys: Vec<Vec<*mut c_char>>,
}

// SAFETY: every pointer in the node is into heap memory the node owns and nothing else touches;
// mpv only reads it during a call.
unsafe impl Send for OwnedNode {}

impl OwnedNode {
    /// The node for `value`. A number with no fraction that fits is an `INT64`, as mpv's own
    /// JSON parser makes one; `null` is `NONE`. A string with a NUL in it cannot be said to mpv,
    /// and is cut at the NUL.
    pub fn from_json(value: &Value) -> Self {
        let mut owned = Self {
            node: none(),
            _strings: Vec::new(),
            _lists: Vec::new(),
            _values: Vec::new(),
            _keys: Vec::new(),
        };
        owned.node = owned.build(value);
        owned
    }

    /// A pointer for a call that reads the node and does not keep it.
    pub fn as_ptr(&mut self) -> *mut mpv_node {
        &mut self.node
    }

    fn string(&mut self, text: &str) -> *mut c_char {
        let text = text.split('\0').next().unwrap_or_default();
        let c = CString::new(text).unwrap_or_default();
        // The CString's buffer is on the heap: moving the CString into the Vec does not move it.
        let ptr = c.as_ptr().cast_mut();
        self._strings.push(c);
        ptr
    }

    fn build(&mut self, value: &Value) -> mpv_node {
        match value {
            Value::Null => none(),
            Value::Bool(b) => {
                mpv_node { u: mpv_node_u { flag: i32::from(*b) }, format: MPV_FORMAT_FLAG }
            }
            Value::Number(n) => match n.as_i64() {
                Some(i) => mpv_node { u: mpv_node_u { int64: i }, format: MPV_FORMAT_INT64 },
                None => mpv_node {
                    u: mpv_node_u { double_: n.as_f64().unwrap_or(0.0) },
                    format: MPV_FORMAT_DOUBLE,
                },
            },
            Value::String(s) => {
                let string = self.string(s);
                mpv_node { u: mpv_node_u { string }, format: MPV_FORMAT_STRING }
            }
            Value::Array(items) => {
                let values: Vec<mpv_node> = items.iter().map(|v| self.build(v)).collect();
                self.list(values, None, MPV_FORMAT_NODE_ARRAY)
            }
            Value::Object(map) => {
                let mut keys = Vec::with_capacity(map.len());
                let mut values = Vec::with_capacity(map.len());
                for (key, value) in map {
                    keys.push(self.string(key));
                    values.push(self.build(value));
                }
                self.list(values, Some(keys), MPV_FORMAT_NODE_MAP)
            }
        }
    }

    fn list(
        &mut self,
        mut values: Vec<mpv_node>,
        keys: Option<Vec<*mut c_char>>,
        format: i32,
    ) -> mpv_node {
        let num = i32::try_from(values.len()).unwrap_or(i32::MAX);
        let mut list = Box::new(mpv_node_list {
            num,
            values: values.as_mut_ptr(),
            keys: std::ptr::null_mut(),
        });
        if let Some(mut keys) = keys {
            list.keys = keys.as_mut_ptr();
            self._keys.push(keys);
        }
        self._values.push(values);
        let ptr: *mut mpv_node_list = &mut *list;
        self._lists.push(list);
        mpv_node { u: mpv_node_u { list: ptr }, format }
    }
}

fn none() -> mpv_node {
    mpv_node { u: mpv_node_u { int64: 0 }, format: MPV_FORMAT_NONE }
}

/// The JSON for a node mpv gave: its value as the engine reads a property. A byte array has no
/// JSON form and is `null`, as is a number JSON cannot hold.
///
/// # Safety
///
/// `node` must be a valid node, as mpv hands one out, for the length of the call.
pub unsafe fn to_json(node: &mpv_node) -> Value {
    // SAFETY: the format says which member of the union is set, and the caller vouches for the
    // node; a list's `num` entries, and its keys for a map, are valid with it.
    unsafe {
        match node.format {
            MPV_FORMAT_STRING => Value::String(text(node.u.string)),
            MPV_FORMAT_FLAG => Value::Bool(node.u.flag != 0),
            MPV_FORMAT_INT64 => Value::Number(node.u.int64.into()),
            MPV_FORMAT_DOUBLE => {
                Number::from_f64(node.u.double_).map_or(Value::Null, Value::Number)
            }
            MPV_FORMAT_NODE_ARRAY | MPV_FORMAT_NODE_MAP => {
                let list = node.u.list;
                if list.is_null() {
                    return Value::Null;
                }
                let list = &*list;
                let len = usize::try_from(list.num).unwrap_or(0);
                let values: &[mpv_node] =
                    if len == 0 { &[] } else { std::slice::from_raw_parts(list.values, len) };
                if node.format == MPV_FORMAT_NODE_ARRAY {
                    Value::Array(values.iter().map(|v| to_json(v)).collect())
                } else {
                    let keys = if len == 0 {
                        &[][..]
                    } else {
                        std::slice::from_raw_parts(list.keys, len)
                    };
                    let mut map = Map::new();
                    for (key, value) in keys.iter().zip(values) {
                        map.insert(text(*key), to_json(value));
                    }
                    Value::Object(map)
                }
            }
            MPV_FORMAT_BYTE_ARRAY | MPV_FORMAT_NONE => Value::Null,
            _ => Value::Null,
        }
    }
}

/// A C string mpv gave, as Rust text, with anything not UTF-8 replaced.
///
/// # Safety
///
/// `ptr` is null or a valid NUL-terminated string.
pub unsafe fn text(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    // SAFETY: the caller vouches for it.
    unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn round_trip(value: Value) -> Value {
        let node = OwnedNode::from_json(&value);
        // SAFETY: the node is owned and alive for the call.
        unsafe { to_json(&node.node) }
    }

    #[test]
    fn a_chapter_list_comes_back_as_it_went() {
        let chapters = json!([
            {"title": "Opening", "time": 0},
            {"title": "Part A", "time": 91.5},
            {"title": "Ending", "time": 1320.25},
        ]);
        assert_eq!(round_trip(chapters.clone()), chapters);
    }

    #[test]
    fn scalars_keep_their_kind() {
        for value in [
            json!(true),
            json!(false),
            json!(42),
            json!(-7),
            json!(0.5),
            json!("a b"),
            json!(null),
        ] {
            assert_eq!(round_trip(value.clone()), value);
        }
        // A whole number goes as an integer, as mpv's own parser makes one.
        let node = OwnedNode::from_json(&json!(3));
        assert_eq!(node.node.format, MPV_FORMAT_INT64);
        let node = OwnedNode::from_json(&json!(3.25));
        assert_eq!(node.node.format, MPV_FORMAT_DOUBLE);
    }

    #[test]
    fn nested_maps_and_empty_lists_survive() {
        let value = json!({"name": "loadfile", "url": "/media/episode.mkv", "flags": "replace",
            "options": {"start": "12.5", "title": "Episode 3"}, "empty": [], "none": {}});
        assert_eq!(round_trip(value.clone()), value);
    }

    #[test]
    fn a_nul_cuts_a_string_rather_than_failing_it() {
        assert_eq!(round_trip(json!("before\0after")), json!("before"));
    }

    #[test]
    fn a_number_json_cannot_hold_is_null() {
        let node = mpv_node { u: mpv_node_u { double_: f64::NAN }, format: MPV_FORMAT_DOUBLE };
        // SAFETY: a scalar node, alive for the call.
        assert_eq!(unsafe { to_json(&node) }, Value::Null);
    }
}
