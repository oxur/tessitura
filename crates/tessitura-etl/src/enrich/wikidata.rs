//! Wikidata enrichment stage.
//!
//! Queries the Wikidata SPARQL endpoint and REST API to fetch structured
//! metadata for musical works that have been linked via a MusicBrainz work
//! ID. The enricher extracts properties such as tonality (key), form,
//! catalog code, instrumentation, time period, and artistic movement
//! (school). All findings are stored as provenance-tracked [`Assertion`]s.
//!
//! [`Assertion`]: tessitura_core::provenance::Assertion

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use tessitura_core::provenance::{Assertion, Source};
use tessitura_core::schema::Database;

use crate::enrich::resilience::RateLimiter;
use crate::error::{EnrichError, EnrichResult};

// ---------------------------------------------------------------------------
// Wikidata property IDs for music works
// ---------------------------------------------------------------------------

/// Tonality (key) -- entity reference (e.g. Q189524 = "A minor").
const PROP_TONALITY: &str = "P826";

/// Form of creative work -- entity reference.
const PROP_FORM: &str = "P7937";

/// Catalog code -- string value.
const PROP_CATALOG: &str = "P528";

/// Instrumentation -- entity reference.
const PROP_INSTRUMENTATION: &str = "P870";

/// Time period -- entity reference.
const PROP_PERIOD: &str = "P2348";

/// Movement (artistic school) -- entity reference.
const PROP_MOVEMENT: &str = "P135";

// ---------------------------------------------------------------------------
// SPARQL response types (private)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct SparqlResult {
    results: SparqlBindings,
}

#[derive(Debug, Deserialize)]
struct SparqlBindings {
    bindings: Vec<HashMap<String, SparqlValue>>,
}

#[derive(Debug, Deserialize)]
struct SparqlValue {
    value: String,
}

// ---------------------------------------------------------------------------
// Entity data response types
// ---------------------------------------------------------------------------

/// Wrapper for the Wikidata `Special:EntityData` JSON response.
#[derive(Debug, Deserialize)]
struct EntityDataWrapper {
    entities: HashMap<String, WikidataEntity>,
}

/// A Wikidata entity with its claims (property-value pairs).
#[derive(Debug, Clone, Deserialize)]
pub struct WikidataEntity {
    /// The QID of this entity (e.g. "Q12345").
    pub id: String,

    /// Property claims keyed by property ID (e.g. "P826").
    #[serde(default)]
    pub claims: HashMap<String, Vec<WikidataClaim>>,
}

/// A single claim (statement) on a Wikidata entity.
#[derive(Debug, Clone, Deserialize)]
pub struct WikidataClaim {
    /// The main snak holding the claim's value.
    pub mainsnak: WikidataSnak,
}

/// The snak (property-value cell) inside a claim.
#[derive(Debug, Clone, Deserialize)]
pub struct WikidataSnak {
    /// The data value, if present. Some snaks may have `snaktype: "novalue"`
    /// or `"somevalue"`, in which case `datavalue` is absent.
    pub datavalue: Option<WikidataDataValue>,
}

/// A typed data value from a Wikidata snak.
///
/// Wikidata encodes values as `{"type": "<type>", "value": <payload>}`.
/// We use serde's internally-tagged representation to dispatch on the
/// `type` field.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum WikidataDataValue {
    /// A plain string value.
    #[serde(rename = "string")]
    StringValue(String),

    /// A reference to another Wikidata entity.
    #[serde(rename = "wikibase-entityid")]
    EntityId(WikidataEntityRef),

    /// A quantity value (amount with optional unit).
    #[serde(rename = "quantity")]
    Quantity(WikidataQuantity),

    /// Any other value type we do not handle explicitly.
    #[serde(other)]
    Other,
}

/// A reference to another Wikidata entity inside a data value.
#[derive(Debug, Clone, Deserialize)]
pub struct WikidataEntityRef {
    /// The QID (e.g. "Q189524").
    pub id: String,
}

/// A quantity value inside a data value.
#[derive(Debug, Clone, Deserialize)]
pub struct WikidataQuantity {
    /// The numeric amount as a string (may include a leading "+").
    pub amount: String,
}

impl WikidataEntity {
    /// Extract plain string values for the given property.
    ///
    /// Returns an empty `Vec` when the property is absent or contains no
    /// string-typed claims.
    pub fn get_string_values(&self, property: &str) -> Vec<String> {
        self.claims
            .get(property)
            .map(|claims| {
                claims
                    .iter()
                    .filter_map(|c| match &c.mainsnak.datavalue {
                        Some(WikidataDataValue::StringValue(s)) => Some(s.clone()),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Extract entity-reference QIDs for the given property.
    ///
    /// Returns an empty `Vec` when the property is absent or contains no
    /// entity-id-typed claims.
    pub fn get_entity_refs(&self, property: &str) -> Vec<String> {
        self.claims
            .get(property)
            .map(|claims| {
                claims
                    .iter()
                    .filter_map(|c| match &c.mainsnak.datavalue {
                        Some(WikidataDataValue::EntityId(e)) => Some(e.id.clone()),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Wikidata SPARQL and REST API client.
///
/// Wraps a `reqwest::Client` pre-configured with the project user-agent and
/// a per-source [`RateLimiter`] (~5 req/sec, well within the Wikidata
/// query service guidelines).
///
/// [`RateLimiter`]: crate::enrich::resilience::RateLimiter
#[derive(Debug, Clone)]
pub struct WikidataClient {
    http: Client,
    rate_limiter: RateLimiter,
}

impl WikidataClient {
    /// Create a new Wikidata client.
    ///
    /// # Errors
    /// Returns an error if the HTTP client cannot be created.
    pub fn new() -> EnrichResult<Self> {
        let http = Client::builder()
            .user_agent("tessitura/0.1.0 (https://github.com/oxur/tessitura)")
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(EnrichError::from)?;

        Ok(Self {
            http,
            rate_limiter: RateLimiter::new(5),
        })
    }

    /// Find a Wikidata QID for a MusicBrainz work ID using SPARQL.
    ///
    /// Queries the Wikidata SPARQL endpoint for entities that have the
    /// `P435` (MusicBrainz work ID) property set to the given value.
    /// Returns `Ok(None)` when no matching entity is found.
    ///
    /// # Errors
    /// Returns an error on HTTP failure or if the response cannot be parsed.
    pub async fn find_by_mb_work_id(&self, mb_work_id: &str) -> EnrichResult<Option<String>> {
        self.rate_limiter.acquire().await;

        let query = format!(
            r#"SELECT ?item WHERE {{ ?item wdt:P435 "{}" }} LIMIT 1"#,
            mb_work_id
        );

        let response = self
            .http
            .get("https://query.wikidata.org/sparql")
            .query(&[("query", query.as_str()), ("format", "json")])
            .send()
            .await?
            .error_for_status()
            .map_err(|e| EnrichError::Http {
                source_name: "Wikidata".to_string(),
                message: e.to_string(),
            })?;

        let result: SparqlResult = response.json().await.map_err(|e| EnrichError::Parse {
            source_name: "Wikidata".to_string(),
            message: e.to_string(),
        })?;

        Ok(result
            .results
            .bindings
            .first()
            .and_then(|binding| binding.get("item"))
            .and_then(|item| item.value.rsplit('/').next())
            .map(String::from))
    }

    /// Fetch entity data for a Wikidata QID.
    ///
    /// Uses the `Special:EntityData` endpoint to retrieve the full JSON
    /// representation of a Wikidata entity, including all claims.
    ///
    /// # Errors
    /// Returns an error on HTTP failure, parse failure, or when the entity
    /// is not found in the response.
    pub async fn get_entity(&self, qid: &str) -> EnrichResult<WikidataEntity> {
        self.rate_limiter.acquire().await;

        let url = format!(
            "https://www.wikidata.org/wiki/Special:EntityData/{}.json",
            qid
        );

        let response = self
            .http
            .get(&url)
            .send()
            .await?
            .error_for_status()
            .map_err(|e| EnrichError::Http {
                source_name: "Wikidata".to_string(),
                message: e.to_string(),
            })?;

        let wrapper: EntityDataWrapper = response.json().await.map_err(|e| EnrichError::Parse {
            source_name: "Wikidata".to_string(),
            message: e.to_string(),
        })?;

        wrapper
            .entities
            .into_values()
            .next()
            .ok_or(EnrichError::NotFound {
                entity: qid.to_string(),
                source_name: "Wikidata".to_string(),
            })
    }
}

// ---------------------------------------------------------------------------
// Enricher
// ---------------------------------------------------------------------------

/// Enriches items with structured metadata from Wikidata.
///
/// The enricher wraps a [`WikidataClient`] and performs a two-step lookup:
/// first it resolves a MusicBrainz work ID to a Wikidata QID via SPARQL,
/// then it fetches the entity data and extracts key music-work properties.
/// Each extracted value is stored as a provenance-tracked [`Assertion`].
///
/// [`Assertion`]: tessitura_core::provenance::Assertion
#[derive(Debug, Clone)]
pub struct WikidataEnricher {
    client: WikidataClient,
}

impl WikidataEnricher {
    /// Create a new Wikidata enricher.
    ///
    /// Constructs an HTTP client and a rate limiter that enforces roughly
    /// 5 requests per second, in line with Wikidata query service guidelines.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP client cannot be created.
    pub fn new() -> EnrichResult<Self> {
        let client = WikidataClient::new()?;
        Ok(Self { client })
    }

    /// Enrich a work by its MusicBrainz work ID.
    ///
    /// Looks up the Wikidata entity linked to the MB work ID (via property
    /// P435), then extracts key, form, catalog code, instrumentation,
    /// period, and school. All findings are stored as provenance-tracked
    /// assertions in the database.
    ///
    /// Returns the list of assertions that were created, or an empty `Vec`
    /// when no Wikidata entity is linked to the given work ID.
    ///
    /// # Errors
    /// Returns an error on HTTP failure, parse failure, or database write
    /// failure.
    pub async fn enrich(
        &self,
        mb_work_id: &str,
        entity_id: &str,
        db_path: &Path,
    ) -> EnrichResult<Vec<Assertion>> {
        // 1. Find Wikidata QID via MusicBrainz work ID (P435)
        let Some(qid) = self.client.find_by_mb_work_id(mb_work_id).await? else {
            log::debug!("No Wikidata entity found for MB work {}", mb_work_id);
            return Ok(Vec::new());
        };

        log::info!("Found Wikidata entity {} for MB work {}", qid, mb_work_id);

        // 2. Fetch entity data
        let entity = self.client.get_entity(&qid).await?;

        // 3. Extract properties into assertions
        let mut assertions = Vec::new();

        // P826 -- Tonality (key)
        for key_ref in entity.get_entity_refs(PROP_TONALITY) {
            assertions.push(
                Assertion::new(
                    entity_id,
                    "key",
                    serde_json::json!({ "wikidata_qid": key_ref }),
                    Source::Wikidata,
                )
                .with_confidence(0.9),
            );
        }

        // P7937 -- Form of creative work
        for form_ref in entity.get_entity_refs(PROP_FORM) {
            assertions.push(
                Assertion::new(
                    entity_id,
                    "form",
                    serde_json::json!({ "wikidata_qid": form_ref }),
                    Source::Wikidata,
                )
                .with_confidence(0.9),
            );
        }

        // P528 -- Catalog code
        for catalog in entity.get_string_values(PROP_CATALOG) {
            assertions.push(
                Assertion::new(
                    entity_id,
                    "catalog_number",
                    serde_json::json!(catalog),
                    Source::Wikidata,
                )
                .with_confidence(0.95),
            );
        }

        // P870 -- Instrumentation
        for instrument_ref in entity.get_entity_refs(PROP_INSTRUMENTATION) {
            assertions.push(
                Assertion::new(
                    entity_id,
                    "instrumentation",
                    serde_json::json!({ "wikidata_qid": instrument_ref }),
                    Source::Wikidata,
                )
                .with_confidence(0.9),
            );
        }

        // P2348 -- Time period
        for period_ref in entity.get_entity_refs(PROP_PERIOD) {
            assertions.push(
                Assertion::new(
                    entity_id,
                    "period",
                    serde_json::json!({ "wikidata_qid": period_ref }),
                    Source::Wikidata,
                )
                .with_confidence(0.85),
            );
        }

        // P135 -- Movement (school)
        for movement_ref in entity.get_entity_refs(PROP_MOVEMENT) {
            assertions.push(
                Assertion::new(
                    entity_id,
                    "school",
                    serde_json::json!({ "wikidata_qid": movement_ref }),
                    Source::Wikidata,
                )
                .with_confidence(0.85),
            );
        }

        // 4. Persist all assertions to the database
        let db = Database::open(db_path)?;
        for assertion in &assertions {
            db.insert_assertion(assertion)?;
        }

        Ok(assertions)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wikidata_client_creation_succeeds() {
        let client = WikidataClient::new();
        assert!(client.is_ok());
    }

    #[test]
    fn test_wikidata_client_debug_format() {
        let client = WikidataClient::new().unwrap();
        let debug = format!("{:?}", client);
        assert!(debug.contains("WikidataClient"));
        assert!(debug.contains("RateLimiter"));
    }

    #[test]
    fn test_wikidata_enricher_creation_succeeds() {
        let enricher = WikidataEnricher::new();
        assert!(enricher.is_ok());
    }

    #[test]
    fn test_wikidata_enricher_debug_format() {
        let enricher = WikidataEnricher::new().unwrap();
        let debug = format!("{:?}", enricher);
        assert!(debug.contains("WikidataEnricher"));
        assert!(debug.contains("WikidataClient"));
    }

    #[test]
    fn test_sparql_result_deserialize_with_binding() {
        let json = r#"{
            "results": {
                "bindings": [
                    {
                        "item": {
                            "type": "uri",
                            "value": "http://www.wikidata.org/entity/Q123456"
                        }
                    }
                ]
            }
        }"#;

        let result: SparqlResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.results.bindings.len(), 1);

        let qid = result.results.bindings[0]
            .get("item")
            .unwrap()
            .value
            .rsplit('/')
            .next()
            .unwrap();
        assert_eq!(qid, "Q123456");
    }

    #[test]
    fn test_sparql_result_deserialize_empty_bindings() {
        let json = r#"{
            "results": {
                "bindings": []
            }
        }"#;

        let result: SparqlResult = serde_json::from_str(json).unwrap();
        assert!(result.results.bindings.is_empty());
    }

    #[test]
    fn test_entity_deserialize_with_string_claims() {
        let json = r#"{
            "id": "Q12345",
            "claims": {
                "P528": [
                    {
                        "mainsnak": {
                            "datavalue": {
                                "type": "string",
                                "value": "Op. 18"
                            }
                        }
                    },
                    {
                        "mainsnak": {
                            "datavalue": {
                                "type": "string",
                                "value": "Sz. 40"
                            }
                        }
                    }
                ]
            }
        }"#;

        let entity: WikidataEntity = serde_json::from_str(json).unwrap();
        assert_eq!(entity.id, "Q12345");

        let values = entity.get_string_values("P528");
        assert_eq!(values, vec!["Op. 18", "Sz. 40"]);
    }

    #[test]
    fn test_entity_deserialize_with_entity_ref_claims() {
        let json = r#"{
            "id": "Q12345",
            "claims": {
                "P826": [
                    {
                        "mainsnak": {
                            "datavalue": {
                                "type": "wikibase-entityid",
                                "value": {
                                    "entity-type": "item",
                                    "numeric-id": 189524,
                                    "id": "Q189524"
                                }
                            }
                        }
                    }
                ]
            }
        }"#;

        let entity: WikidataEntity = serde_json::from_str(json).unwrap();
        let refs = entity.get_entity_refs("P826");
        assert_eq!(refs, vec!["Q189524"]);
    }

    #[test]
    fn test_entity_get_string_values_empty_when_property_absent() {
        let json = r#"{"id": "Q12345", "claims": {}}"#;
        let entity: WikidataEntity = serde_json::from_str(json).unwrap();
        assert!(entity.get_string_values("P528").is_empty());
    }

    #[test]
    fn test_entity_get_entity_refs_empty_when_property_absent() {
        let json = r#"{"id": "Q12345", "claims": {}}"#;
        let entity: WikidataEntity = serde_json::from_str(json).unwrap();
        assert!(entity.get_entity_refs("P826").is_empty());
    }

    #[test]
    fn test_entity_get_string_values_skips_non_string_claims() {
        let json = r#"{
            "id": "Q12345",
            "claims": {
                "P528": [
                    {
                        "mainsnak": {
                            "datavalue": {
                                "type": "wikibase-entityid",
                                "value": {
                                    "entity-type": "item",
                                    "numeric-id": 999,
                                    "id": "Q999"
                                }
                            }
                        }
                    }
                ]
            }
        }"#;

        let entity: WikidataEntity = serde_json::from_str(json).unwrap();
        assert!(entity.get_string_values("P528").is_empty());
    }

    #[test]
    fn test_entity_get_entity_refs_skips_non_entity_claims() {
        let json = r#"{
            "id": "Q12345",
            "claims": {
                "P826": [
                    {
                        "mainsnak": {
                            "datavalue": {
                                "type": "string",
                                "value": "not an entity"
                            }
                        }
                    }
                ]
            }
        }"#;

        let entity: WikidataEntity = serde_json::from_str(json).unwrap();
        assert!(entity.get_entity_refs("P826").is_empty());
    }

    #[test]
    fn test_entity_with_missing_datavalue() {
        let json = r#"{
            "id": "Q12345",
            "claims": {
                "P826": [
                    {
                        "mainsnak": {}
                    }
                ]
            }
        }"#;

        let entity: WikidataEntity = serde_json::from_str(json).unwrap();
        assert!(entity.get_entity_refs("P826").is_empty());
        assert!(entity.get_string_values("P826").is_empty());
    }

    #[test]
    fn test_entity_with_no_claims() {
        let json = r#"{"id": "Q12345"}"#;
        let entity: WikidataEntity = serde_json::from_str(json).unwrap();
        assert!(entity.claims.is_empty());
        assert!(entity.get_string_values("P528").is_empty());
        assert!(entity.get_entity_refs("P826").is_empty());
    }

    #[test]
    fn test_entity_data_wrapper_deserialize() {
        let json = r#"{
            "entities": {
                "Q12345": {
                    "id": "Q12345",
                    "claims": {}
                }
            }
        }"#;

        let wrapper: EntityDataWrapper = serde_json::from_str(json).unwrap();
        assert_eq!(wrapper.entities.len(), 1);
        assert!(wrapper.entities.contains_key("Q12345"));
        assert_eq!(wrapper.entities["Q12345"].id, "Q12345");
    }

    #[test]
    fn test_quantity_datavalue_deserialize() {
        let json = r#"{
            "id": "Q12345",
            "claims": {
                "P999": [
                    {
                        "mainsnak": {
                            "datavalue": {
                                "type": "quantity",
                                "value": {
                                    "amount": "+4",
                                    "unit": "1"
                                }
                            }
                        }
                    }
                ]
            }
        }"#;

        let entity: WikidataEntity = serde_json::from_str(json).unwrap();
        let claims = entity.claims.get("P999").unwrap();
        assert_eq!(claims.len(), 1);

        match &claims[0].mainsnak.datavalue {
            Some(WikidataDataValue::Quantity(q)) => assert_eq!(q.amount, "+4"),
            other => panic!("expected Quantity, got {:?}", other),
        }
    }

    #[test]
    fn test_entity_multiple_properties() {
        let json = r#"{
            "id": "Q42",
            "claims": {
                "P826": [
                    {
                        "mainsnak": {
                            "datavalue": {
                                "type": "wikibase-entityid",
                                "value": { "entity-type": "item", "numeric-id": 1, "id": "Q1" }
                            }
                        }
                    }
                ],
                "P528": [
                    {
                        "mainsnak": {
                            "datavalue": {
                                "type": "string",
                                "value": "BWV 1048"
                            }
                        }
                    }
                ],
                "P7937": [
                    {
                        "mainsnak": {
                            "datavalue": {
                                "type": "wikibase-entityid",
                                "value": { "entity-type": "item", "numeric-id": 2, "id": "Q2" }
                            }
                        }
                    }
                ],
                "P870": [
                    {
                        "mainsnak": {
                            "datavalue": {
                                "type": "wikibase-entityid",
                                "value": { "entity-type": "item", "numeric-id": 3, "id": "Q3" }
                            }
                        }
                    },
                    {
                        "mainsnak": {
                            "datavalue": {
                                "type": "wikibase-entityid",
                                "value": { "entity-type": "item", "numeric-id": 4, "id": "Q4" }
                            }
                        }
                    }
                ],
                "P2348": [
                    {
                        "mainsnak": {
                            "datavalue": {
                                "type": "wikibase-entityid",
                                "value": { "entity-type": "item", "numeric-id": 5, "id": "Q5" }
                            }
                        }
                    }
                ],
                "P135": [
                    {
                        "mainsnak": {
                            "datavalue": {
                                "type": "wikibase-entityid",
                                "value": { "entity-type": "item", "numeric-id": 6, "id": "Q6" }
                            }
                        }
                    }
                ]
            }
        }"#;

        let entity: WikidataEntity = serde_json::from_str(json).unwrap();

        assert_eq!(entity.get_entity_refs("P826"), vec!["Q1"]);
        assert_eq!(entity.get_string_values("P528"), vec!["BWV 1048"]);
        assert_eq!(entity.get_entity_refs("P7937"), vec!["Q2"]);
        assert_eq!(entity.get_entity_refs("P870"), vec!["Q3", "Q4"]);
        assert_eq!(entity.get_entity_refs("P2348"), vec!["Q5"]);
        assert_eq!(entity.get_entity_refs("P135"), vec!["Q6"]);
    }
}
