//! Custom `serde` deserializer for `FilterMatch`

use core::fmt;
use core::str::FromStr;

use ibc::core::ics24_host::identifier::{ChannelId, PortId};
use itertools::Itertools;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

/// Represents the ways in which packets can be filtered.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    rename_all = "lowercase",
    tag = "policy",
    content = "list",
    deny_unknown_fields
)]
pub enum PacketFilter {
    /// Allow packets from the specified channels.
    Allow(ChannelFilters),
    /// Deny packets from the specified channels.
    Deny(ChannelFilters),
    /// Allow any & all packets.
    AllowAll,
}

impl Default for PacketFilter {
    /// By default, allows all channels & ports.
    fn default() -> Self {
        Self::AllowAll
    }
}

impl PacketFilter {
    /// Returns true if the packets can be relayed on the channel with [`PortId`] and [`ChannelId`],
    /// false otherwise.
    pub fn is_allowed(&self, port_id: &PortId, channel_id: &ChannelId) -> bool {
        match self {
            PacketFilter::Allow(spec) => spec.matches(&(port_id.clone(), channel_id.clone())),
            PacketFilter::Deny(spec) => !spec.matches(&(port_id.clone(), channel_id.clone())),
            PacketFilter::AllowAll => true,
        }
    }
}

/// The internal representation of channel filter policies.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelFilters(Vec<(PortFilterMatch, ChannelFilterMatch)>);

impl ChannelFilters {
    /// Indicates whether a match for the given [`PortId`]-[`ChannelId`] pair
    /// exists in the filter policy.
    pub fn matches(&self, channel_port: &(PortId, ChannelId)) -> bool {
        let (port_id, channel_id) = channel_port;
        self.0.iter().any(|(port_filter, chan_filter)| {
            port_filter.matches(port_id) && chan_filter.matches(channel_id)
        })
    }

    /// Indicates whether this filter policy contains only exact patterns.
    #[inline]
    pub fn is_exact(&self) -> bool {
        self.0.iter().all(|(port_filter, channel_filter)| {
            port_filter.is_exact() && channel_filter.is_exact()
        })
    }

    /// An iterator over the [`PortId`]-[`ChannelId`] pairs that don't contain wildcards.
    pub fn iter_exact(&self) -> impl Iterator<Item = (&PortId, &ChannelId)> {
        self.0.iter().filter_map(|port_chan_filter| {
            if let &(FilterPattern::Exact(ref port_id), FilterPattern::Exact(ref chan_id)) =
                port_chan_filter
            {
                Some((port_id, chan_id))
            } else {
                None
            }
        })
    }
}

impl fmt::Display for ChannelFilters {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.0
                .iter()
                .map(|(pid, cid)| format!("{}/{}", pid, cid))
                .join(", ")
        )
    }
}

impl Serialize for ChannelFilters {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeSeq;

        struct Pair<'a> {
            a: &'a FilterPattern<PortId>,
            b: &'a FilterPattern<ChannelId>,
        }

        impl<'a> Serialize for Pair<'a> {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                let mut seq = serializer.serialize_seq(Some(2))?;
                seq.serialize_element(self.a)?;
                seq.serialize_element(self.b)?;
                seq.end()
            }
        }

        let mut outer_seq = serializer.serialize_seq(Some(self.0.len()))?;

        for (port, channel) in &self.0 {
            outer_seq.serialize_element(&Pair {
                a: port,
                b: channel,
            })?;
        }

        outer_seq.end()
    }
}

/// Newtype wrapper for expressing wildcard patterns compiled to a [`regex::Regex`].
#[derive(Clone, Debug)]
pub struct Wildcard(regex::Regex);

impl Wildcard {
    #[inline]
    pub fn is_match(&self, text: &str) -> bool {
        self.0.is_match(text)
    }
}

impl FromStr for Wildcard {
    type Err = regex::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let regex = regex::escape(s).replace("\\*", "(?:.*)").parse()?;
        Ok(Self(regex))
    }
}

impl fmt::Display for Wildcard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.0.to_string().replace("(?:.*)", "*");
        write!(f, "{}", s)
    }
}

impl Serialize for Wildcard {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Represents a single channel to be filtered in a [`ChannelFilters`] list.
#[derive(Clone, Debug)]
pub enum FilterPattern<T> {
    /// A channel specified exactly with its [`PortId`] & [`ChannelId`].
    Exact(T),
    /// A glob of channel(s) specified with a wildcard in either or both [`PortId`] & [`ChannelId`].
    Wildcard(Wildcard),
}

impl<T> FilterPattern<T> {
    /// Indicates whether this filter is specified in part with a wildcard.
    pub fn is_wildcard(&self) -> bool {
        matches!(self, Self::Wildcard(_))
    }

    /// Indicates whether this filter is specified as an exact match.
    pub fn is_exact(&self) -> bool {
        matches!(self, Self::Exact(_))
    }

    /// Matches the given value via strict equality if the filter is an `Exact`, or via
    /// wildcard matching if the filter is a `Pattern`.
    pub fn matches(&self, value: &T) -> bool
    where
        T: PartialEq + AsRef<str>,
    {
        match self {
            FilterPattern::Exact(v) => value == v,
            FilterPattern::Wildcard(regex) => regex.is_match(value.as_ref()),
        }
    }

    /// Returns the contained value if this filter contains an `Exact` variant, or
    /// `None` if it contains a `Pattern`.
    pub fn exact_value(&self) -> Option<&T> {
        match self {
            FilterPattern::Exact(value) => Some(value),
            FilterPattern::Wildcard(_) => None,
        }
    }
}

impl<T: fmt::Display> fmt::Display for FilterPattern<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FilterPattern::Exact(value) => write!(f, "{}", value),
            FilterPattern::Wildcard(regex) => write!(f, "{}", regex),
        }
    }
}

impl<T> Serialize for FilterPattern<T>
where
    T: AsRef<str>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            FilterPattern::Exact(e) => serializer.serialize_str(e.as_ref()),
            FilterPattern::Wildcard(t) => serializer.serialize_str(&t.to_string()),
        }
    }
}

/// Type alias for a [`FilterPattern`] containing a [`PortId`].
pub type PortFilterMatch = FilterPattern<PortId>;
/// Type alias for a [`FilterMatch`] containing a [`ChannelId`].
pub type ChannelFilterMatch = FilterPattern<ChannelId>;

impl<'de> Deserialize<'de> for PortFilterMatch {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<PortFilterMatch, D::Error> {
        deserializer.deserialize_string(port::PortFilterMatchVisitor)
    }
}

impl<'de> Deserialize<'de> for ChannelFilterMatch {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<ChannelFilterMatch, D::Error> {
        deserializer.deserialize_string(channel::ChannelFilterMatchVisitor)
    }
}

pub(crate) mod port {
    use super::*;
    use ibc::core::ics24_host::identifier::PortId;

    pub struct PortFilterMatchVisitor;

    impl<'de> de::Visitor<'de> for PortFilterMatchVisitor {
        type Value = PortFilterMatch;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("valid PortId or wildcard")
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            if let Ok(port_id) = PortId::from_str(v) {
                Ok(PortFilterMatch::Exact(port_id))
            } else {
                let wildcard = v.parse().map_err(E::custom)?;
                Ok(PortFilterMatch::Wildcard(wildcard))
            }
        }

        fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
            self.visit_str(&v)
        }
    }
}

pub(crate) mod channel {
    use super::*;
    use ibc::core::ics24_host::identifier::ChannelId;

    pub struct ChannelFilterMatchVisitor;

    impl<'de> de::Visitor<'de> for ChannelFilterMatchVisitor {
        type Value = ChannelFilterMatch;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("valid ChannelId or wildcard")
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            if let Ok(channel_id) = ChannelId::from_str(v) {
                Ok(ChannelFilterMatch::Exact(channel_id))
            } else {
                let wildcard = v.parse().map_err(E::custom)?;
                Ok(ChannelFilterMatch::Wildcard(wildcard))
            }
        }

        fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
            self.visit_str(&v)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PacketFilter;

    #[test]
    fn deserialize_packet_filter_policy() {
        let toml_content = r#"
            policy = 'allow'
            list = [
              ['ica*', '*'],
              ['transfer', 'channel-0'],
            ]
            "#;

        let filter_policy: PacketFilter =
            toml::from_str(toml_content).expect("could not parse filter policy");

        dbg!(filter_policy);
    }

    #[test]
    fn serialize_packet_filter_policy() {
        use std::str::FromStr;

        use ibc::core::ics24_host::identifier::{ChannelId, PortId};

        let filter_policy = ChannelFilters(vec![
            (
                FilterPattern::Exact(PortId::from_str("transfer").unwrap()),
                FilterPattern::Exact(ChannelId::from_str("channel-0").unwrap()),
            ),
            (
                FilterPattern::Wildcard("ica*".parse().unwrap()),
                FilterPattern::Wildcard("*".parse().unwrap()),
            ),
        ]);

        let fp = PacketFilter::Allow(filter_policy);
        let toml_str = toml::to_string_pretty(&fp).expect("could not serialize packet filter");

        println!("{}", toml_str);
    }
}
