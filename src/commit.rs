use crate::error::{Error, ErrorKind};
use crate::object_id::ObjectId;

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct Signature {
    pub name: Vec<u8>,
    pub email: Vec<u8>,
    pub seconds: i64,
    pub offset_minutes: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct CommitRecord {
    pub id: ObjectId,
    pub tree: ObjectId,
    pub parents: Vec<ObjectId>,
    pub author: Signature,
    pub committer: Signature,
    pub message: Vec<u8>,
    pub encoding: Option<Vec<u8>>,
}

pub(crate) fn read(repository: &git2::Repository, id: ObjectId) -> Result<CommitRecord, Error> {
    let native = repository
        .find_commit(id.to_git2())
        .map_err(|error| Error::from_native(ErrorKind::ObjectRead, error))?;
    let odb = repository
        .odb()
        .map_err(|error| Error::from_native(ErrorKind::ObjectRead, error))?;
    let object = odb
        .read(id.to_git2())
        .map_err(|error| Error::from_native(ErrorKind::ObjectRead, error))?;
    let (headers, message) = split_headers(object.data());
    let author = native_signature(native.author());
    let committer = native_signature(native.committer());
    let encoding = find_optional_header(headers, b"encoding ").map(ToOwned::to_owned);
    let parents = native
        .parent_ids()
        .map(ObjectId::from_git2)
        .collect::<Vec<_>>();
    Ok(CommitRecord {
        id,
        tree: ObjectId::from_git2(native.tree_id()),
        parents,
        author,
        committer,
        message: message.to_vec(),
        encoding,
    })
}

fn native_signature(signature: git2::Signature<'_>) -> Signature {
    Signature {
        name: signature.name_bytes().to_vec(),
        email: signature.email_bytes().to_vec(),
        seconds: signature.when().seconds(),
        offset_minutes: signature.when().offset_minutes(),
    }
}

fn split_headers(data: &[u8]) -> (&[u8], &[u8]) {
    match data.windows(2).position(|window| window == b"\n\n") {
        Some(separator) => (&data[..separator], &data[separator + 2..]),
        None => (data, &[]),
    }
}

fn find_optional_header<'a>(headers: &'a [u8], prefix: &[u8]) -> Option<&'a [u8]> {
    for line in headers.split(|byte| *byte == b'\n') {
        if line.starts_with(prefix) {
            return Some(&line[prefix.len()..]);
        }
    }
    None
}
