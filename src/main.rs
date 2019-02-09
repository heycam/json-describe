use clap::{App, Arg, crate_description, crate_name, crate_version};
use std::cmp;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::iter::Peekable;
use std::result;
use json_tools::{Buffer, BufferType, Lexer, Token as JsonToken, TokenType};

type Result<T> = result::Result<T, ()>;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    CurlyOpen,
    CurlyClose,
    BracketOpen,
    BracketClose,
    Colon,
    Comma,
    String(String),
    Boolean,
    Number(String),
    Null,
}

impl Token {
    fn from_json_token(token: JsonToken) -> Token {
        match token.kind {
            TokenType::CurlyOpen => Token::CurlyOpen,
            TokenType::CurlyClose => Token::CurlyClose,
            TokenType::BracketOpen => Token::BracketOpen,
            TokenType::BracketClose => Token::BracketClose,
            TokenType::Colon => Token::Colon,
            TokenType::Comma => Token::Comma,
            TokenType::BooleanTrue | TokenType::BooleanFalse => Token::Boolean,
            TokenType::String | TokenType::Number => {
                let s = match token.buf {
                    Buffer::MultiByte(bytes) => String::from_utf8(bytes).unwrap(),
                    Buffer::Span(..) => panic!("unexpected Span"),
                };
                if token.kind == TokenType::String {
                    Token::String(s)
                } else {
                    Token::Number(s)
                }
            }
            TokenType::Null => Token::Null,
            TokenType::Invalid => panic!("invalid token"),
        }
    }
}

#[derive(Clone)]
enum Value {
    String(HashSet<String>, bool),
    Boolean,
    Number(HashSet<String>, bool),
    Null,
    Object(HashMap<String, (Vec<Value>, bool)>),
    Array(Vec<Value>, usize, usize),
}

const MAX_EXAMPLES: usize = 4;

impl Value {
    fn from_token(kind: Token) -> Value {
        let is_string = match &kind {
            Token::String(..) => true,
            _ => false,
        };
        match kind {
            Token::String(s) | Token::Number(s) => {
                let mut examples = HashSet::new();
                examples.insert(s);
                if is_string {
                    Value::String(examples, false)
                } else {
                    Value::Number(examples, false)
                }
            }
            Token::Boolean => Value::Boolean,
            Token::Null => Value::Null,
            Token::CurlyOpen => Value::Object(Default::default()),
            Token::BracketOpen => Value::Array(Default::default(), 0, 0),
            _ => panic!("unexpected token: {:?}", kind),
        }
    }

    fn sort_key(&self) -> usize {
        match self {
            Value::Null => 1,
            Value::Boolean => 2,
            Value::Number(..) => 3,
            Value::String(..) => 4,
            Value::Array(..) => 5,
            Value::Object(..) => 6,
        }
    }

    fn merge_with(&self, other: &Value) -> Result<Value> {
        match (self, other) {
            (Value::String(ex1, more1), Value::String(ex2, _)) |
            (Value::Number(ex1, more1), Value::Number(ex2, _)) => {
                let mut examples = ex1.clone();
                let mut more = *more1;
                for ex in ex2 {
                    if examples.len() >= MAX_EXAMPLES {
                        if !examples.contains(ex) {
                            more = true;
                        }
                        break;
                    }
                    examples.insert(ex.clone());
                }
                if let Value::String(..) = self {
                    Ok(Value::String(examples, more))
                } else {
                    Ok(Value::Number(examples, more))
                }
            }
            (Value::Boolean, Value::Boolean) |
            (Value::Null, Value::Null) => Ok(self.clone()),
            (Value::Object(self_pairs), Value::Object(other_pairs)) => {
                let mut all_keys = HashMap::<&str, (bool, bool)>::new();
                for k in self_pairs.keys() {
                    all_keys.entry(k).or_default().0 = true;
                }
                for k in other_pairs.keys() {
                    all_keys.entry(k).or_default().1 = true;
                }
                let mut new_pairs = HashMap::new();
                for (k, (in_self, in_other)) in all_keys {
                    let mut new_values;
                    let new_missing;
                    if in_self && in_other {
                        let entry = self_pairs.get(k).unwrap().clone();
                        new_values = entry.0;
                        new_missing = entry.1;
                        for other_value in &other_pairs.get(k).unwrap().0 {
                            let mut merged = false;
                            for v in new_values.iter_mut() {
                                if let Ok(merged_value) = v.merge_with(&other_value) {
                                    *v = merged_value;
                                    merged = true;
                                    break;
                                }
                            }
                            if !merged {
                                new_values.push(other_value.clone());
                            }
                        }
                    } else {
                        let source = if in_self { self_pairs } else { other_pairs };
                        new_values = source.get(k).unwrap().0.clone();
                        new_missing = true;
                    }
                    new_pairs.insert(k.to_owned(), (new_values, new_missing));
                }
                Ok(Value::Object(new_pairs))
            }
            (Value::Array(self_values, self_min, self_max), Value::Array(other_values, other_min, other_max)) => {
                let mut new_values = self_values.clone();
                for other_value in other_values {
                    let mut merged = false;
                    for v in new_values.iter_mut() {
                        if let Ok(merged_value) = v.merge_with(other_value) {
                            *v = merged_value;
                            merged = true;
                            break;
                        }
                    }
                    if !merged {
                        new_values.push(other_value.clone());
                    }
                }
                Ok(Value::Array(new_values, cmp::min(*self_min, *other_min), cmp::max(*self_max, *other_max)))
            }
            _ => Err(()),
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::String(ex, more) | Value::Number(ex, more) => {
                if let Value::String(..) = self {
                    f.write_str("String (")?;
                } else {
                    f.write_str("Number (")?;
                }
                let mut examples = ex.iter().collect::<Vec<_>>();
                examples.sort();
                let mut first = true;
                for example in examples {
                    if !first {
                        f.write_str(", ")?;
                    }
                    f.write_str(example)?;
                    first = false;
                }
                if *more {
                    f.write_str(", ...")?;
                }
                f.write_str(")")
            }
            Value::Boolean => f.write_str("Boolean"),
            Value::Null => f.write_str("null"),
            Value::Object(pairs) => {
                let mut debug_map = f.debug_map();
                let mut pairs = pairs.iter().collect::<Vec<_>>();
                pairs.sort_unstable_by(|(k1, _), (k2, _)| k1.partial_cmp(k2).unwrap());
                for (k, (vs, missing)) in pairs {
                    debug_map.entry(k, &ValuesFormat(vs, *missing));
                }
                debug_map.finish()
            }
            Value::Array(values, min, max) => {
                if min == max {
                    write!(f, "Array (len {}) ", min)?;
                } else {
                    write!(f, "Array (len {}..{}) ", min, max)?;
                }
                let mut values = values.clone();
                values.sort_unstable_by_key(|v| v.sort_key());
                f.debug_list().entries(values.iter()).finish()
            }
        }
    }
}

struct ValuesFormat<'a, T: fmt::Debug>(&'a [T], bool);

impl<'a, T: fmt::Debug> fmt::Debug for ValuesFormat<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.1 {
            f.write_str("optional ")?;
        }
        if self.0.len() == 1 {
            self.0[0].fmt(f)
        } else {
            let mut debug_tuple = f.debug_tuple("");
            for v in self.0 {
                debug_tuple.field(v);
            }
            debug_tuple.finish()
        }
    }
}

struct InputIterator<I>(Peekable<I>)
where
    I: Iterator,
    I::Item: PartialEq + fmt::Debug;

impl<I> InputIterator<I>
where
    I: Iterator,
    I::Item: PartialEq + fmt::Debug,
{
    fn new(iter: I) -> InputIterator<I> {
        InputIterator(iter.peekable())
    }

    fn peek(&mut self) -> Option<&I::Item> {
        self.0.peek()
    }

    fn expect(&mut self, value: I::Item) -> bool {
        let found = self.peek() == Some(&value);
        if found {
            self.next();
        }
        found
    }
}

impl<I> Iterator for InputIterator<I>
where
    I: Iterator,
    I::Item: PartialEq + fmt::Debug,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let t = self.0.next();
        t
    }
}

fn parse_value<I>(tokens: &mut InputIterator<I>) -> Value
where
    I: Iterator<Item = Token>,
{
    let mut value = Value::from_token(tokens.next().unwrap());
    match value {
        Value::Object(ref mut pairs) => {
            if !tokens.expect(Token::CurlyClose) {
                loop {
                    let key = tokens.next().unwrap();
                    if let Token::String(key) = key {
                        let key = key[1..key.len() - 1].to_string();
                        if pairs.contains_key(&key) {
                            panic!("duplicate key: {}", key);
                        }
                        if !tokens.expect(Token::Colon) {
                            panic!("unexpected token: {:?}", tokens.next());
                        }
                        let value = parse_value(tokens);
                        pairs.insert(key, (vec![value], false));
                        match tokens.next().unwrap() {
                            Token::Comma => continue,
                            Token::CurlyClose => break,
                            t => panic!("unexpected token: {:?}", t),
                        }
                    } else {
                        panic!("unexpected token: {:?}", key);
                    }
                }
            }
        }
        Value::Array(ref mut values, ref mut min, ref mut max) => {
            let mut n = 0;
            if !tokens.expect(Token::BracketClose) {
                loop {
                    let value = parse_value(tokens);
                    n += 1;
                    let mut merged = false;
                    for v in values.iter_mut() {
                        if let Ok(merged_value) = v.merge_with(&value) {
                            *v = merged_value;
                            merged = true;
                            break;
                        }
                    }
                    if !merged {
                        values.push(value);
                    }
                    match tokens.next().unwrap() {
                        Token::Comma => continue,
                        Token::BracketClose => break,
                        t => panic!("unexpected token: {:?}", t),
                    }
                }
            }
            *min = n;
            *max = n;
        }
        _ => {}
    }
    value
}

fn describe<R: Read>(input: R) {
    let lexer = Lexer::new(input.bytes().map(|b| b.unwrap()), BufferType::Bytes(8));
    let value = parse_value(&mut InputIterator::new(lexer.map(Token::from_json_token)));
    println!("{:#?}", value);
}

fn main() {
    let matches = App::new(crate_name!())
        .version(crate_version!())
        .about(crate_description!())
        .arg(
            Arg::with_name("FILE")
                .help("specifies the input JSON file")
                .index(1)
        )
        .get_matches();

    match matches.value_of("FILE") {
        None | Some("-") => describe(io::stdin()),
        Some(filename) => {
            let file = match File::open(filename) {
                Ok(f) => f,
                Err(e) => panic!("could not open file {}: {}", filename, e),
            };
            describe(file)
        }
    }
}
