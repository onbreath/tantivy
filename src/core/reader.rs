use core::index::{Segment, SegmentId};
use core::schema::Term;
use core::store::StoreReader;
use core::schema::Document;
use core::postings::IntersectionPostings;
use byteorder::{BigEndian, ReadBytesExt};
use core::directory::ReadOnlySource;
use std::io::Cursor;
use core::schema::DocId;
use core::index::SegmentComponent;
use core::postings::Postings;
use core::simdcompression::Decoder;
use std::io;
use core::codec::TermInfo;
use core::fstmap::FstMap;
use core::serial::SegmentSerializer;
use core::serial::SerializableSegment;

// TODO file structure should be in codec

pub struct SegmentReader {
    segment: Segment,
    term_offsets: FstMap<TermInfo>,
    postings_data: ReadOnlySource,
    store_reader: StoreReader,
}

pub struct SegmentPostings {
    doc_id: usize,
    doc_ids: Vec<u32>,
}

impl SegmentPostings {

    pub fn empty()-> SegmentPostings {
        SegmentPostings {
            doc_id: 0,
            doc_ids: Vec::new(),
        }
    }

    pub fn from_data(data: &[u8]) -> SegmentPostings {
        let mut cursor = Cursor::new(data);
        let data_size = cursor.read_u32::<BigEndian>().unwrap() as usize;
        // TODO remove allocs
        let mut data = Vec::with_capacity(data_size);
        for _ in 0..data_size {
            data.push(cursor.read_u32::<BigEndian>().unwrap());
        }
        let mut doc_ids: Vec<u32> = (0..10_000_000 as u32).collect();
        let decoder = Decoder::new();
        let num_doc_ids = decoder.decode_sorted(&data, &mut doc_ids);
        doc_ids.truncate(num_doc_ids);
        SegmentPostings {
            doc_ids: doc_ids,
            doc_id: 0,
        }
    }

}

impl Postings for SegmentPostings {
    fn skip_next(&mut self, target: DocId) -> Option<DocId> {
        loop {
            match Iterator::next(self) {
                Some(val) if val >= target => {
                    return Some(val);
                },
                None => {
                    return None;
                },
                _ => {}
            }
        }
    }
}


impl Iterator for SegmentPostings {

    type Item = DocId;

    fn next(&mut self,) -> Option<DocId> {
        if self.doc_id < self.doc_ids.len() {
            let res = Some(self.doc_ids[self.doc_id]);
            self.doc_id += 1;
            return res;
        }
        else {
            None
        }
    }
}


impl SegmentReader {

    pub fn id(&self,) -> SegmentId {
        self.segment.id()
    }

    pub fn open(segment: Segment) -> io::Result<SegmentReader> {
        let source = try!(segment.open_read(SegmentComponent::TERMS));
        let term_offsets = try!(FstMap::from_source(source));
        let store_reader = StoreReader::new(try!(segment.open_read(SegmentComponent::STORE)));
        let postings_shared_mmap = try!(segment.open_read(SegmentComponent::POSTINGS));
        Ok(SegmentReader {
            postings_data: postings_shared_mmap,
            term_offsets: term_offsets,
            segment: segment,
            store_reader: store_reader,
        })
    }

    pub fn get_doc(&self, doc_id: &DocId) -> Document {
        self.store_reader.get(doc_id)
    }

    fn read_postings(&self, offset: usize) -> SegmentPostings {
        let postings_data = &self.postings_data.as_slice()[offset..];
        SegmentPostings::from_data(&postings_data)
    }

    pub fn get_term<'a>(&'a self, term: &Term) -> Option<TermInfo> {
        self.term_offsets.get(term.as_slice())
    }

    pub fn search(&self, terms: &Vec<Term>) -> IntersectionPostings<SegmentPostings> {

        let mut segment_postings: Vec<SegmentPostings> = Vec::new();
        for term in terms.iter() {
            match self.get_term(term) {
                Some(term_info) => {
                    let segment_posting = self.read_postings(term_info.postings_offset as usize);
                    segment_postings.push(segment_posting);
                }
                None => {
                    segment_postings.clear();
                    segment_postings.push(SegmentPostings::empty());
                    break;
                }
            }
        }
        IntersectionPostings::from_postings(segment_postings)
    }

}

//
// fn write_postings<R: io::Read, Output, SegSer: SegmentSerializer<Output>>(mut cursor: R, num_docs: DocId, serializer: &mut SegSer) -> io::Result<()> {
//     // TODO remove allocation
//     let docs = Vec::with_capacity(num_docs as usize);
//     for i in 0..num_docs {
//         let doc_id = u32::serialize(&mut cursor);
//         try!(serializer.add_doc(doc_id));
//     }
//     Ok(())
// }
//
// impl SerializableSegment for SegmentReader {
//
//     fn write<Output, SegSer: SegmentSerializer<Output>>(&self, mut serializer: SegSer) -> io::Result<Output> {
//         let mut term_offsets_it = self.term_offsets.stream();
//         loop {
//             match term_offsets_it.next() {
//                 Some((term_data, offset_u64)) => {
//                     let term = Term::from(term_data);
//                     let offset = offset_u64 as usize;
//                     try!(serializer.new_term(&term, num_docs));
//                     let segment_postings = self.read_postings(offset);
//                     try!(write_postings(cursor, num_docs, &mut serializer));
//                 },
//                 None => { break; }
//             }
//         }
//         serializer.close()
//     }
//
// }
