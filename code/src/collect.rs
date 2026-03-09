use std::path::Path;
use std::io::BufWriter;

use rust_code_analysis::{
    CcommentParser, CodeMetrics, CppParser, FuncSpace, GoParser, HaskellParser, JavaParser, JavascriptParser, KotlinParser, LANG, ParserTrait, PreprocParser, PythonParser, RustParser, ScalaParser, SpaceKind, SwiftParser, TsxParser, TypescriptParser, metrics
};
use serde::ser::SerializeSeq;
use serde::{Serialize, Serializer};
use walkdir::WalkDir;

#[derive(Debug, Serialize)]
pub enum Metric {
    NARGS, NEXITS, COGNITIVE, CYCLOMATIC, HALSTEAD, LOC, NOM, MI, ABC, WMC, NPM, NPA, SAFE,
}

#[derive(Debug, Serialize)]
pub enum Qualifier {
    FILE, FUNCTION, UNKNOWN, CLASS, STRUCT, TRAIT, IMPL, UNIT, NAMESPACE, INTERFACE,
}

impl Qualifier {
    pub fn from_kind(kind: &SpaceKind) -> Self {
        match kind {
            SpaceKind::Function => Qualifier::FUNCTION,
            SpaceKind::Class => Qualifier::CLASS,
            SpaceKind::Struct => Qualifier::STRUCT,
            SpaceKind::Trait => Qualifier::TRAIT,
            SpaceKind::Impl => Qualifier::IMPL,
            SpaceKind::Unit => Qualifier::UNIT,
            SpaceKind::Namespace => Qualifier::NAMESPACE,
            SpaceKind::Interface => Qualifier::INTERFACE,
            _ => Qualifier::UNKNOWN,
        }
    }
}

#[derive(Debug)]
pub struct ComponentMetrics(pub CodeMetrics);

impl Serialize for ComponentMetrics {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(13))?;
        seq.serialize_element(&(Metric::NARGS, &self.0.nargs))?;
        seq.serialize_element(&(Metric::NEXITS, &self.0.nexits))?;
        seq.serialize_element(&(Metric::COGNITIVE, &self.0.cognitive))?;
        seq.serialize_element(&(Metric::CYCLOMATIC, &self.0.cyclomatic))?;
        seq.serialize_element(&(Metric::HALSTEAD, &self.0.halstead))?;
        seq.serialize_element(&(Metric::LOC, &self.0.loc))?;
        seq.serialize_element(&(Metric::NOM, &self.0.nom))?;
        seq.serialize_element(&(Metric::MI, &self.0.mi))?;
        seq.serialize_element(&(Metric::ABC, &self.0.abc))?;
        seq.serialize_element(&(Metric::WMC, &self.0.wmc))?;
        seq.serialize_element(&(Metric::NPM, &self.0.npm))?;
        seq.serialize_element(&(Metric::NPA, &self.0.npa))?;
        seq.serialize_element(&(Metric::SAFE, &self.0.safecheck))?;
        seq.end()
    }
}

#[derive(Debug, Serialize)]
pub struct Component {
    measurements: ComponentMetrics,
    qualifier: Qualifier,
    name: String,
    path: String,
    language: Option<&'static str>, 
    start_line: Option<usize>,
    end_line: Option<usize>,
}

#[derive(Debug)]
pub enum CollectError {
    FileError,
    AnalysisError,
}

pub fn parse(lang: LANG, source: Vec<u8>, path: &std::path::Path) -> Option<FuncSpace> {
    match lang {
        LANG::Javascript => metrics(&JavascriptParser::new(source, path, None), &path),
        LANG::Java => metrics(&JavaParser::new(source, path, None), &path),
        LANG::Kotlin => metrics(&KotlinParser::new(source, path, None), &path),
        LANG::Rust => metrics(&RustParser::new(source, path, None), &path),
        LANG::Cpp => metrics(&CppParser::new(source, path, None), &path),
        LANG::Python => metrics(&PythonParser::new(source, path, None), &path),
        LANG::Tsx => metrics(&TsxParser::new(source, path, None), &path),
        LANG::Typescript => metrics(&TypescriptParser::new(source, path, None), &path),
        LANG::Ccomment => metrics(&CcommentParser::new(source, path, None), &path),
        LANG::Preproc => metrics(&PreprocParser::new(source, path, None), &path),
        LANG::Go => metrics(&GoParser::new(source, path, None), &path),
        LANG::Haskell => metrics(&HaskellParser::new(source, path, None), &path),
        LANG::Swift => metrics(&SwiftParser::new(source, path, None), &path),
        LANG::Scala => metrics(&ScalaParser::new(source, path, None), &path),
        _ => None,
    }
}

fn collect_func_space(
    space: FuncSpace,
    lang: LANG,
    is_root: bool,
    path_buf: &mut String,
    out: &mut Vec<Component>,
) {
    let original_len = path_buf.len();

    let name = if is_root {
        path_buf.split('/').last().unwrap_or("UNKNOWN").to_string()
    } else {
        space.name.unwrap_or_else(|| "UNKNOWN".to_string())
    };

    if !is_root {
        path_buf.push_str("::");
        path_buf.push_str(&name);
    }

    out.push(Component {
        qualifier: Qualifier::from_kind(&space.kind),
        name,
        path: path_buf.clone(),
        language: Some(lang.get_name()),
        start_line: Some(space.start_line),
        end_line: Some(space.end_line),
        measurements: ComponentMetrics(space.metrics),
    });

    for child in space.spaces {
        collect_func_space(child, lang, false, path_buf, out);
    }

    path_buf.truncate(original_len);
}

pub fn collect_repository(folder: &str) -> Result<Vec<Component>, CollectError> {
    let mut components = Vec::with_capacity(10_000);

    for entry in WalkDir::new(folder).into_iter().filter_map(Result::ok) {
        if entry.file_type().is_file() {
            let path = entry.path();

            if let Some(language) = rust_code_analysis::get_language_for_file(path) {
                if let Ok(source) = rust_code_analysis::read_file(path) {
                    if let Some(func_space) = parse(language, source, path) {
                        let path_rel = path.strip_prefix(folder).unwrap_or(path).to_str().unwrap_or("UNKNOWN");
                        
                        let mut path_buf = path_rel.to_string();
                        collect_func_space(func_space, language, true, &mut path_buf, &mut components);
                    }
                }
            }
        }
    }

    Ok(components)
}

const DEFAULT_BUFFER_SIZE: usize = 1024 * 1024 * 4; // 4 MB

pub fn save_components(path: &Path, components: &[Component]) -> Result<(), CollectError> {
    let file = std::fs::File::create(path).map_err(|_| CollectError::FileError)?;
    
    let file_writer = BufWriter::with_capacity(DEFAULT_BUFFER_SIZE, file);
    
    let encoder = flate2::write::GzEncoder::new(file_writer, flate2::Compression::new(4));
    
    let encoder_writer = BufWriter::with_capacity(DEFAULT_BUFFER_SIZE, encoder);
    serde_json::to_writer(encoder_writer, components).map_err(|_| CollectError::AnalysisError)?;

    Ok(())
}