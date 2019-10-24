// Copyright 2019 Joyent, Inc.

use crate::util;
use base64;
use diesel::backend;
use diesel::deserialize::{self, FromSql};
use diesel::serialize::{self, IsNull, Output, ToSql};
use diesel::sql_types;
use diesel::sqlite::Sqlite;
use md5;
use quickcheck::{Arbitrary, Gen};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::io::Write;
use uuid::Uuid;

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
#[serde(tag = "type")]
pub enum ObjectType {
    #[serde(alias = "object")]
    Object(MantaObject),

    #[serde(alias = "directory")]
    Directory(MantaDirectory),
}

#[derive(
    Deserialize,
    Serialize,
    Default,
    PartialEq,
    Debug,
    Clone,
    FromSqlRow,
    AsExpression,
)]
#[serde(rename_all = "camelCase")]
#[sql_type = "sql_types::Text"]
pub struct MantaObject {
    pub headers: Value,
    pub key: String,
    pub mtime: u64,
    pub name: String,
    pub creator: String,
    pub dirname: String,
    pub owner: String,
    pub roles: Vec<String>, // TODO: double check this is a String
    pub vnode: u64,

    #[serde(alias = "contentLength", default)]
    pub content_length: u64,

    #[serde(alias = "contentMD5", default)]
    pub content_md5: String,

    #[serde(alias = "contentType", default)]
    pub content_type: String,

    #[serde(alias = "objectId", default)]
    pub object_id: String,

    #[serde(default)]
    pub etag: String,

    #[serde(default)]
    pub sharks: Vec<MantaObjectShark>,

    #[serde(alias = "type", default)]
    pub obj_type: String,
}

impl ToSql<sql_types::Text, Sqlite> for MantaObject {
    fn to_sql<W: Write>(
        &self,
        out: &mut Output<W, Sqlite>,
    ) -> serialize::Result {
        let manta_str = serde_json::to_string(&self).unwrap();
        out.write_all(manta_str.as_bytes())?;

        Ok(IsNull::No)
    }
}

impl FromSql<sql_types::Text, Sqlite> for MantaObject {
    fn from_sql(
        bytes: Option<backend::RawValue<Sqlite>>,
    ) -> deserialize::Result<Self> {
        let manta_obj: MantaObject =
            serde_json::from_str(not_none!(bytes).read_text())?;
        Ok(manta_obj)
    }
}

#[derive(Deserialize, Serialize, Default, PartialEq, Debug, Clone)]
pub struct MantaObjectShark {
    pub datacenter: String,
    pub manta_storage_id: String,
}

#[derive(Deserialize, Default, Serialize, PartialEq, Debug, Clone)]
pub struct MantaDirectory {
    pub creator: String,
    pub dirname: String,
    pub headers: Value,
    pub key: String,
    pub mtime: u64,
    pub name: String,
    pub owner: String,
    pub roles: Vec<String>, // TODO: double check this is a String
    pub vnode: u64,
}

// Implement Arbitrary traits for testing
impl Arbitrary for MantaObjectShark {
    fn arbitrary<G: Gen>(g: &mut G) -> MantaObjectShark {
        let len = g.gen_range(1, 100) as usize;
        let msid = format!(
            "{}.{}.{}",
            len,
            util::random_string(g, len),
            util::random_string(g, len)
        );
        MantaObjectShark {
            datacenter: util::random_string(g, len),
            manta_storage_id: msid,
        }
    }
}

impl Arbitrary for MantaObject {
    fn arbitrary<G: Gen>(g: &mut G) -> MantaObject {
        let len = g.gen::<u8>() as usize;

        let mut headers_map = Map::new();
        headers_map.insert(
            util::random_string(g, len),
            Value::String(util::random_string(g, len)),
        );

        headers_map.insert(
            util::random_string(g, len),
            Value::String(util::random_string(g, len)),
        );

        headers_map.insert(
            util::random_string(g, len),
            Value::String(util::random_string(g, len)),
        );

        let headers = Value::Object(headers_map);
        let key = util::random_string(g, len);
        let mtime: u64 = g.gen();
        let creator = util::random_string(g, len);
        let dirname = util::random_string(g, len);
        let name = util::random_string(g, len);
        let owner = Uuid::new_v4().to_string();
        let roles: Vec<String> = vec![util::random_string(g, len)];
        let vnode: u64 = g.gen();
        let content_length: u64 = g.gen();

        let md5_sum = md5::compute(util::random_string(g, len));
        let content_md5: String = base64::encode(&*md5_sum);

        let etag: String = Uuid::new_v4().to_string();
        let content_type: String = util::random_string(g, len);
        let object_id: String = Uuid::new_v4().to_string();
        let sharks: Vec<MantaObjectShark> = vec![
            MantaObjectShark::arbitrary(g),
            MantaObjectShark::arbitrary(g),
        ];
        let obj_type = String::from("object");

        MantaObject {
            headers,
            key,
            mtime,
            name,
            dirname,
            creator,
            owner,
            roles,
            vnode,
            content_length,
            content_md5,
            content_type,
            object_id,
            etag,
            sharks,
            obj_type,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck::quickcheck;
    use regex::Regex;
    use std::str::FromStr;

    quickcheck!(
        fn create_manta_object(mobj: MantaObject) -> bool {
            dbg!(&mobj);

            let str_etag = Uuid::from_str(mobj.etag.as_str());
            let str_owner = Uuid::from_str(mobj.owner.as_str());
            let str_object_id = Uuid::from_str(mobj.object_id.as_str());
            assert!(str_etag.is_ok());
            assert!(str_owner.is_ok());
            assert!(str_object_id.is_ok());

            assert_eq!(str_etag.unwrap().to_string(), mobj.etag);
            assert_eq!(str_owner.unwrap().to_string(), mobj.owner);
            assert_eq!(str_object_id.unwrap().to_string(), mobj.object_id);

            let re = Regex::new(r"(?i)\d+.[a-z0-9-]+.[a-z0-9-]+").unwrap();

            for shark in mobj.sharks.iter() {
                dbg!(&shark.manta_storage_id);
                assert!(re.is_match(&shark.manta_storage_id));
            }

            true
        }
    );
}
