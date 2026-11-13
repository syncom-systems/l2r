//! Identities of the versioned generation profiles a model may select.
//!
//! A profile fixes both the kind of a component and the exact files generated
//! for it. Changing what a profile generates requires a new profile version.

/// A pure state machine: owned state, typed inputs and outputs, and a total
/// transition function implemented by the implementer.
pub const TRANSITION_CORE_V0: &str = "transition-core/v0";

/// A container that owns no behavioral state and composes its children through
/// explicit FIFO mailboxes and approved message edges.
pub const COMPOSITE_V0: &str = "composite/v0";

/// A container whose children have bounded mailboxes (capacity is part of the
/// semantics, `l2r.async/v1`), with typed input ports and a scheduler whose
/// deliveries are enabled only when every receiver has room. `composite/v0` is
/// unchanged and remains available.
pub const COMPOSITE_V1: &str = "composite/v1";

/// A pure state machine whose Lean state, input and output types may carry
/// data (structures, constructors with fields, fixed-width integers). Its Rust
/// types are generated once per Lean type in the crate's `types` module. Model
/// schema `v2`.
pub const TRANSITION_CORE_V1: &str = "transition-core/v1";

/// A composite that may be nested and has typed input and output ports. Its
/// behaviour is that of the flat network of the machines below it
/// (`l2r.async/v1`); the root composite generates the scheduler. Model schema
/// `v2`.
pub const COMPOSITE_V2: &str = "composite/v2";

/// Every profile this generator version implements.
pub const SUPPORTED: &[&str] = &[
    COMPOSITE_V0,
    COMPOSITE_V1,
    COMPOSITE_V2,
    TRANSITION_CORE_V0,
    TRANSITION_CORE_V1,
];

use crate::architecture::{MODEL_SCHEMA, MODEL_SCHEMA_V1, MODEL_SCHEMA_V2};

/// What a component is: a state machine or a composite of other components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// Owns state and changes it only through its Lean transition.
    Machine,
    /// Owns no behavioural state; composes its children.
    Composite,
}

/// A model schema version. The model's `schema` string is parsed into one, in
/// one place; every decision about a schema is a `match` on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Schema {
    /// `l2r.architecture/v0`.
    V0,
    /// `l2r.architecture/v1`: adds ports, mailbox capacities and `composite/v1`.
    V1,
    /// `l2r.architecture/v2`: hierarchy and data; its own profiles only.
    V2,
}

/// A field of the model format that only some schemas admit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Feature {
    /// `[[port]]` tables.
    Ports,
    /// `mailbox_capacity` of a component.
    MailboxCapacity,
    /// `direction` of a port.
    PortDirection,
    /// `capacity` of a port.
    PortCapacity,
}

impl Schema {
    /// Every schema this generator accepts, oldest first.
    pub const ALL: [Schema; 3] = [Schema::V0, Schema::V1, Schema::V2];

    /// The schema a model's `schema` string names, if this generator knows it.
    pub fn parse(id: &str) -> Option<Schema> {
        Schema::ALL.into_iter().find(|s| s.id() == id)
    }

    /// The identifier written in models.
    pub fn id(self) -> &'static str {
        match self {
            Schema::V0 => MODEL_SCHEMA,
            Schema::V1 => MODEL_SCHEMA_V1,
            Schema::V2 => MODEL_SCHEMA_V2,
        }
    }

    /// True when a model of this schema may select `profile`. `v1` adds to
    /// `v0`; `v2` has its own profiles only.
    pub fn admits(self, profile: Profile) -> bool {
        match self {
            Schema::V0 => profile.schema() == Schema::V0,
            Schema::V1 => matches!(profile.schema(), Schema::V0 | Schema::V1),
            Schema::V2 => profile.schema() == Schema::V2,
        }
    }

    /// True when a model of this schema may use `feature`.
    pub fn admits_feature(self, feature: Feature) -> bool {
        match (self, feature) {
            (Schema::V0, _) => false,
            (Schema::V1, Feature::Ports | Feature::MailboxCapacity) => true,
            (Schema::V1, Feature::PortDirection | Feature::PortCapacity) => false,
            (Schema::V2, _) => true,
        }
    }
}

/// A generation profile version. A component's `profile` string is parsed
/// into one, in one place; every decision about a profile is a `match` on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Profile {
    /// `transition-core/v0`.
    TransitionCoreV0,
    /// `composite/v0`.
    CompositeV0,
    /// `composite/v1`.
    CompositeV1,
    /// `transition-core/v1`.
    TransitionCoreV1,
    /// `composite/v2`.
    CompositeV2,
}

impl Profile {
    /// Every profile this generator implements.
    pub const ALL: [Profile; 5] = [
        Profile::TransitionCoreV0,
        Profile::CompositeV0,
        Profile::CompositeV1,
        Profile::TransitionCoreV1,
        Profile::CompositeV2,
    ];

    /// The profile a component's `profile` string names, if this generator
    /// implements it.
    pub fn parse(id: &str) -> Option<Profile> {
        Profile::ALL.into_iter().find(|p| p.id() == id)
    }

    /// The identifier written in models and plans.
    pub fn id(self) -> &'static str {
        match self {
            Profile::TransitionCoreV0 => TRANSITION_CORE_V0,
            Profile::CompositeV0 => COMPOSITE_V0,
            Profile::CompositeV1 => COMPOSITE_V1,
            Profile::TransitionCoreV1 => TRANSITION_CORE_V1,
            Profile::CompositeV2 => COMPOSITE_V2,
        }
    }

    /// Whether components of this profile are machines or composites.
    pub fn kind(self) -> Kind {
        match self {
            Profile::TransitionCoreV0 | Profile::TransitionCoreV1 => Kind::Machine,
            Profile::CompositeV0 | Profile::CompositeV1 | Profile::CompositeV2 => Kind::Composite,
        }
    }

    /// The schema that introduced this profile.
    pub fn schema(self) -> Schema {
        match self {
            Profile::TransitionCoreV0 | Profile::CompositeV0 => Schema::V0,
            Profile::CompositeV1 => Schema::V1,
            Profile::TransitionCoreV1 | Profile::CompositeV2 => Schema::V2,
        }
    }

    /// True when a component of this profile may contain one of `child`.
    pub fn admits_child(self, child: Profile) -> bool {
        match self {
            Profile::TransitionCoreV0 | Profile::TransitionCoreV1 => false,
            Profile::CompositeV0 => {
                matches!(child, Profile::TransitionCoreV0 | Profile::CompositeV0)
            }
            Profile::CompositeV1 => child == Profile::TransitionCoreV0,
            Profile::CompositeV2 => {
                matches!(child, Profile::TransitionCoreV1 | Profile::CompositeV2)
            }
        }
    }
}

impl crate::architecture::Component {
    /// The component's profile, if this generator implements it.
    pub fn parsed_profile(&self) -> Option<Profile> {
        Profile::parse(&self.profile)
    }

    /// The component's kind, if its profile is implemented.
    pub fn kind(&self) -> Option<Kind> {
        self.parsed_profile().map(Profile::kind)
    }
}

impl crate::architecture::Model {
    /// The model's schema, if this generator accepts it.
    pub fn parsed_schema(&self) -> Option<Schema> {
        Schema::parse(&self.schema)
    }

    /// Every schema-gated feature the document uses: its own, its
    /// components' and its ports'.
    pub fn features(&self) -> Vec<Feature> {
        let own = (!self.ports.is_empty()).then_some(Feature::Ports);
        let components = self
            .components
            .iter()
            .filter(|c| c.mailbox_capacity.is_some())
            .map(|_| Feature::MailboxCapacity);
        let ports = self
            .ports
            .iter()
            .flat_map(crate::architecture::Port::features);
        own.into_iter().chain(components).chain(ports).collect()
    }
}

impl crate::architecture::Port {
    /// The schema-gated features this port uses.
    pub fn features(&self) -> Vec<Feature> {
        let direction = self.direction.is_some().then_some(Feature::PortDirection);
        let capacity = self.capacity.is_some().then_some(Feature::PortCapacity);
        direction.into_iter().chain(capacity).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_identifier_parses_back() {
        for profile in Profile::ALL {
            assert_eq!(Profile::parse(profile.id()), Some(profile));
            assert!(SUPPORTED.contains(&profile.id()));
        }
        assert_eq!(Profile::ALL.len(), SUPPORTED.len());
        for schema in Schema::ALL {
            assert_eq!(Schema::parse(schema.id()), Some(schema));
            assert!(crate::SUPPORTED_SCHEMAS.contains(&schema.id()));
        }
        assert_eq!(Schema::ALL.len(), crate::SUPPORTED_SCHEMAS.len());
        assert_eq!(Profile::parse("composite/v9"), None);
        assert_eq!(Schema::parse("l2r.architecture/v9"), None);
    }

    #[test]
    fn schemas_admit_their_features() {
        use Feature::*;
        let admitted = |s: Schema| -> Vec<Feature> {
            [Ports, MailboxCapacity, PortDirection, PortCapacity]
                .into_iter()
                .filter(|f| s.admits_feature(*f))
                .collect()
        };
        assert_eq!(admitted(Schema::V0), []);
        assert_eq!(admitted(Schema::V1), [Ports, MailboxCapacity]);
        assert_eq!(
            admitted(Schema::V2),
            [Ports, MailboxCapacity, PortDirection, PortCapacity]
        );
    }

    #[test]
    fn kinds_schemas_and_children() {
        use Profile::*;
        let machines: Vec<Profile> = Profile::ALL
            .into_iter()
            .filter(|p| p.kind() == Kind::Machine)
            .collect();
        assert_eq!(machines, [TransitionCoreV0, TransitionCoreV1]);
        let introduced: Vec<Schema> = Profile::ALL.into_iter().map(Profile::schema).collect();
        assert_eq!(
            introduced,
            [Schema::V0, Schema::V0, Schema::V1, Schema::V2, Schema::V2]
        );
        let children = |parent: Profile| -> Vec<Profile> {
            Profile::ALL
                .into_iter()
                .filter(|c| parent.admits_child(*c))
                .collect()
        };
        assert_eq!(children(TransitionCoreV0), []);
        assert_eq!(children(TransitionCoreV1), []);
        assert_eq!(children(CompositeV0), [TransitionCoreV0, CompositeV0]);
        assert_eq!(children(CompositeV1), [TransitionCoreV0]);
        assert_eq!(children(CompositeV2), [TransitionCoreV1, CompositeV2]);
    }

    #[test]
    fn a_document_lists_the_gated_features_it_uses() {
        let mut model = crate::load::parse_model(
            "schema = \"l2r.architecture/v1\"\n[system]\nid = \"s\"\nname = \"S\"\nrust_crate = \"s\"\nlean_inventory = \"i.json\"\noutput_dir = \"o\"\n\n[[component]]\nid = \"a\"\nname = \"A\"\nprofile = \"composite/v0\"\nresponsibility = \"r\"\nrust_module = \"a\"\n",
        )
        .expect("parses");
        assert_eq!(model.parsed_schema(), Some(Schema::V1));
        assert_eq!(model.components[0].kind(), Some(Kind::Composite));
        assert_eq!(model.features(), []);
        model.components[0].mailbox_capacity = Some(2);
        assert_eq!(model.features(), [Feature::MailboxCapacity]);
        model.components[0].profile = "composite/v9".into();
        assert_eq!(model.components[0].parsed_profile(), None);
    }

    #[test]
    fn schemas_admit_their_profiles() {
        use Profile::*;
        let admitted = |s: Schema| -> Vec<Profile> {
            Profile::ALL.into_iter().filter(|p| s.admits(*p)).collect()
        };
        assert_eq!(admitted(Schema::V0), [TransitionCoreV0, CompositeV0]);
        assert_eq!(
            admitted(Schema::V1),
            [TransitionCoreV0, CompositeV0, CompositeV1]
        );
        assert_eq!(admitted(Schema::V2), [TransitionCoreV1, CompositeV2]);
    }
}
