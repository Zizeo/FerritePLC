use std::path::Path;

use ferrite_core::{DirectAddress, IecType, IecValue, TagEntry};
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;

use crate::{LanguageKind, PouManifestEntry, PouSource, Project, ProjectError};

/// Implements standard PLCopen TC6 XML (IEC 61131-3) import and export.
pub struct PlcOpenTc6;

impl PlcOpenTc6 {
    /// Exports a `Project` to standard PLCopen TC6 XML 2.01 format.
    pub fn export_project(project: &Project) -> Result<String, ProjectError> {
        let mut buffer = Vec::new();
        let mut writer = Writer::new_with_indent(&mut buffer, b' ', 2);

        // <?xml version="1.0" encoding="utf-8"?>
        writer
            .write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))
            .map_err(|e| ProjectError::Xml(format!("XML decl write error: {e}")))?;

        // <project xmlns="http://www.plcopen.org/xml/tc6_0201">
        let mut project_elem = BytesStart::new("project");
        project_elem.push_attribute(("xmlns", "http://www.plcopen.org/xml/tc6_0201"));
        writer
            .write_event(Event::Start(project_elem))
            .map_err(|e| ProjectError::Xml(format!("XML start error: {e}")))?;

        // <fileHeader companyName="FerritePLC" productName="FerritePLC" productVersion="0.1.0" creationDateTime="2026-01-01T00:00:00"/>
        let mut file_hdr = BytesStart::new("fileHeader");
        file_hdr.push_attribute(("companyName", "FerritePLC"));
        file_hdr.push_attribute(("productName", "FerritePLC"));
        file_hdr.push_attribute(("productVersion", "0.1.0"));
        writer
            .write_event(Event::Empty(file_hdr))
            .map_err(|e| ProjectError::Xml(format!("XML fileHeader error: {e}")))?;

        // <contentHeader name="project.manifest.name">
        let mut content_hdr = BytesStart::new("contentHeader");
        content_hdr.push_attribute(("name", project.manifest.name.as_str()));
        content_hdr.push_attribute(("version", project.manifest.version.as_str()));
        writer
            .write_event(Event::Start(content_hdr))
            .map_err(|e| ProjectError::Xml(format!("XML contentHeader error: {e}")))?;
        writer
            .write_event(Event::End(BytesEnd::new("contentHeader")))
            .map_err(|e| ProjectError::Xml(format!("XML contentHeader end error: {e}")))?;

        // <types><pous>
        writer
            .write_event(Event::Start(BytesStart::new("types")))
            .map_err(|e| ProjectError::Xml(format!("XML types start error: {e}")))?;
        writer
            .write_event(Event::Start(BytesStart::new("pous")))
            .map_err(|e| ProjectError::Xml(format!("XML pous start error: {e}")))?;

        for entry in &project.manifest.pous {
            let mut pou_elem = BytesStart::new("pou");
            pou_elem.push_attribute(("name", entry.name.as_str()));
            pou_elem.push_attribute(("pouType", "program"));
            writer
                .write_event(Event::Start(pou_elem))
                .map_err(|e| ProjectError::Xml(format!("XML pou start error: {e}")))?;

            // <body>
            writer
                .write_event(Event::Start(BytesStart::new("body")))
                .map_err(|e| ProjectError::Xml(format!("XML body start error: {e}")))?;

            if let Some(pou_source) = project.pous.get(&entry.name) {
                match pou_source {
                    PouSource::St(st) => {
                        writer
                            .write_event(Event::Start(BytesStart::new("ST")))
                            .map_err(|e| ProjectError::Xml(format!("XML ST start error: {e}")))?;
                        let mut xhtml = BytesStart::new("xhtml");
                        xhtml.push_attribute(("xmlns", "http://www.w3.org/1999/xhtml"));
                        writer
                            .write_event(Event::Start(xhtml))
                            .map_err(|e| ProjectError::Xml(format!("XML xhtml start error: {e}")))?;
                        writer
                            .write_event(Event::CData(quick_xml::events::BytesCData::new(&st.source)))
                            .map_err(|e| ProjectError::Xml(format!("XML CDATA error: {e}")))?;
                        writer
                            .write_event(Event::End(BytesEnd::new("xhtml")))
                            .map_err(|e| ProjectError::Xml(format!("XML xhtml end error: {e}")))?;
                        writer
                            .write_event(Event::End(BytesEnd::new("ST")))
                            .map_err(|e| ProjectError::Xml(format!("XML ST end error: {e}")))?;
                    }
                    PouSource::Ld(ld) => {
                        writer
                            .write_event(Event::Start(BytesStart::new("LD")))
                            .map_err(|e| ProjectError::Xml(format!("XML LD start error: {e}")))?;
                        let json = serde_json::to_string_pretty(ld)
                            .map_err(|e| ProjectError::Xml(format!("JSON serialization error: {e}")))?;
                        writer
                            .write_event(Event::CData(quick_xml::events::BytesCData::new(json)))
                            .map_err(|e| ProjectError::Xml(format!("XML CDATA error: {e}")))?;
                        writer
                            .write_event(Event::End(BytesEnd::new("LD")))
                            .map_err(|e| ProjectError::Xml(format!("XML LD end error: {e}")))?;
                    }
                    PouSource::Sfc(sfc) => {
                        writer
                            .write_event(Event::Start(BytesStart::new("SFC")))
                            .map_err(|e| ProjectError::Xml(format!("XML SFC start error: {e}")))?;
                        let json = serde_json::to_string_pretty(sfc)
                            .map_err(|e| ProjectError::Xml(format!("JSON serialization error: {e}")))?;
                        writer
                            .write_event(Event::CData(quick_xml::events::BytesCData::new(json)))
                            .map_err(|e| ProjectError::Xml(format!("XML CDATA error: {e}")))?;
                        writer
                            .write_event(Event::End(BytesEnd::new("SFC")))
                            .map_err(|e| ProjectError::Xml(format!("XML SFC end error: {e}")))?;
                    }
                    PouSource::Fbd(fbd) => {
                        writer
                            .write_event(Event::Start(BytesStart::new("FBD")))
                            .map_err(|e| ProjectError::Xml(format!("XML FBD start error: {e}")))?;
                        let json = serde_json::to_string_pretty(fbd)
                            .map_err(|e| ProjectError::Xml(format!("JSON serialization error: {e}")))?;
                        writer
                            .write_event(Event::CData(quick_xml::events::BytesCData::new(json)))
                            .map_err(|e| ProjectError::Xml(format!("XML CDATA error: {e}")))?;
                        writer
                            .write_event(Event::End(BytesEnd::new("FBD")))
                            .map_err(|e| ProjectError::Xml(format!("XML FBD end error: {e}")))?;
                    }
                }
            }

            // </body>
            writer
                .write_event(Event::End(BytesEnd::new("body")))
                .map_err(|e| ProjectError::Xml(format!("XML body end error: {e}")))?;

            // </pou>
            writer
                .write_event(Event::End(BytesEnd::new("pou")))
                .map_err(|e| ProjectError::Xml(format!("XML pou end error: {e}")))?;
        }

        // </pous></types>
        writer
            .write_event(Event::End(BytesEnd::new("pous")))
            .map_err(|e| ProjectError::Xml(format!("XML pous end error: {e}")))?;
        writer
            .write_event(Event::End(BytesEnd::new("types")))
            .map_err(|e| ProjectError::Xml(format!("XML types end error: {e}")))?;

        // <instances><configurations><configuration name="Config0"><resource name="Res0">
        writer
            .write_event(Event::Start(BytesStart::new("instances")))
            .map_err(|e| ProjectError::Xml(format!("XML instances start error: {e}")))?;
        writer
            .write_event(Event::Start(BytesStart::new("configurations")))
            .map_err(|e| ProjectError::Xml(format!("XML configurations start error: {e}")))?;
        let mut cfg = BytesStart::new("configuration");
        cfg.push_attribute(("name", "Config0"));
        writer
            .write_event(Event::Start(cfg))
            .map_err(|e| ProjectError::Xml(format!("XML configuration start error: {e}")))?;
        let mut res = BytesStart::new("resource");
        res.push_attribute(("name", "Res0"));
        writer
            .write_event(Event::Start(res))
            .map_err(|e| ProjectError::Xml(format!("XML resource start error: {e}")))?;

        // <globalVars>
        writer
            .write_event(Event::Start(BytesStart::new("globalVars")))
            .map_err(|e| ProjectError::Xml(format!("XML globalVars start error: {e}")))?;
        for tag in &project.global_tags.tags {
            let mut var_elem = BytesStart::new("variable");
            var_elem.push_attribute(("name", tag.name.as_str()));
            if let Some(addr) = tag.address {
                var_elem.push_attribute(("address", addr.to_string().as_str()));
            }
            writer
                .write_event(Event::Start(var_elem))
                .map_err(|e| ProjectError::Xml(format!("XML variable start error: {e}")))?;

            // <type><BOOL/></type>
            writer
                .write_event(Event::Start(BytesStart::new("type")))
                .map_err(|e| ProjectError::Xml(format!("XML type start error: {e}")))?;
            let type_str = format!("{:?}", tag.data_type).to_uppercase();
            writer
                .write_event(Event::Empty(BytesStart::new(&type_str)))
                .map_err(|e| ProjectError::Xml(format!("XML type empty error: {e}")))?;
            writer
                .write_event(Event::End(BytesEnd::new("type")))
                .map_err(|e| ProjectError::Xml(format!("XML type end error: {e}")))?;

            // <documentation><xhtml>comment</xhtml></documentation>
            if !tag.comment.is_empty() {
                writer
                    .write_event(Event::Start(BytesStart::new("documentation")))
                    .map_err(|e| ProjectError::Xml(format!("XML doc start error: {e}")))?;
                writer
                    .write_event(Event::Text(BytesText::new(&tag.comment)))
                    .map_err(|e| ProjectError::Xml(format!("XML doc text error: {e}")))?;
                writer
                    .write_event(Event::End(BytesEnd::new("documentation")))
                    .map_err(|e| ProjectError::Xml(format!("XML doc end error: {e}")))?;
            }

            writer
                .write_event(Event::End(BytesEnd::new("variable")))
                .map_err(|e| ProjectError::Xml(format!("XML variable end error: {e}")))?;
        }
        writer
            .write_event(Event::End(BytesEnd::new("globalVars")))
            .map_err(|e| ProjectError::Xml(format!("XML globalVars end error: {e}")))?;

        // Tasks: <task name="..." interval="..." priority="...">
        for task in &project.manifest.tasks {
            let mut task_elem = BytesStart::new("task");
            task_elem.push_attribute(("name", task.name.as_str()));
            let prio_str = task.priority.to_string();
            task_elem.push_attribute(("priority", prio_str.as_str()));
            let int_str = format!("T#{}ms", task.interval_ms);
            task_elem.push_attribute(("interval", int_str.as_str()));
            writer
                .write_event(Event::Start(task_elem))
                .map_err(|e| ProjectError::Xml(format!("XML task start error: {e}")))?;

            for pou_name in &task.pou_order {
                let mut pou_inst = BytesStart::new("pouInstance");
                pou_inst.push_attribute(("name", pou_name.as_str()));
                pou_inst.push_attribute(("typeName", pou_name.as_str()));
                writer
                    .write_event(Event::Empty(pou_inst))
                    .map_err(|e| ProjectError::Xml(format!("XML pouInstance error: {e}")))?;
            }

            writer
                .write_event(Event::End(BytesEnd::new("task")))
                .map_err(|e| ProjectError::Xml(format!("XML task end error: {e}")))?;
        }

        // Close resource, configuration, configurations, instances, project
        writer
            .write_event(Event::End(BytesEnd::new("resource")))
            .map_err(|e| ProjectError::Xml(format!("XML resource end error: {e}")))?;
        writer
            .write_event(Event::End(BytesEnd::new("configuration")))
            .map_err(|e| ProjectError::Xml(format!("XML configuration end error: {e}")))?;
        writer
            .write_event(Event::End(BytesEnd::new("configurations")))
            .map_err(|e| ProjectError::Xml(format!("XML configurations end error: {e}")))?;
        writer
            .write_event(Event::End(BytesEnd::new("instances")))
            .map_err(|e| ProjectError::Xml(format!("XML instances end error: {e}")))?;
        writer
            .write_event(Event::End(BytesEnd::new("project")))
            .map_err(|e| ProjectError::Xml(format!("XML project end error: {e}")))?;

        String::from_utf8(buffer)
            .map_err(|e| ProjectError::Xml(format!("Invalid UTF-8 in generated XML: {e}")))
    }

    /// Imports a PLCopen TC6 XML project string and scaffolds a new project directory at `target_dir`.
    pub fn import_project(xml: &str, target_dir: &Path) -> Result<Project, ProjectError> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut project_name = "plcopen_project".to_string();
        let mut global_tags: Vec<TagEntry> = Vec::new();
        let mut imported_pous: Vec<(String, LanguageKind, String)> = Vec::new(); // (name, kind, content)

        let mut buf = Vec::new();
        let mut current_pou_name: Option<String> = None;
        let mut in_body_st = false;
        let mut in_body_ld = false;
        let mut in_body_sfc = false;
        let mut in_body_fbd = false;

        let mut current_var_name: Option<String> = None;
        let mut current_var_addr: Option<DirectAddress> = None;
        let mut current_var_type: Option<IecType> = None;
        let mut current_var_comment = String::new();
        let mut in_type = false;
        let mut in_doc = false;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => match e.name().as_ref() {
                    b"contentHeader" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"name"
                                && let Ok(val) = std::str::from_utf8(&attr.value)
                            {
                                project_name = val.to_string();
                            }
                        }
                    }
                    b"pou" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"name"
                                && let Ok(val) = std::str::from_utf8(&attr.value)
                            {
                                current_pou_name = Some(val.to_string());
                            }
                        }
                    }
                    b"ST" => in_body_st = true,
                    b"LD" => in_body_ld = true,
                    b"SFC" => in_body_sfc = true,
                    b"FBD" => in_body_fbd = true,
                    b"variable" => {
                        current_var_name = None;
                        current_var_addr = None;
                        current_var_type = None;
                        current_var_comment.clear();
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"name" {
                                if let Ok(val) = std::str::from_utf8(&attr.value) {
                                    current_var_name = Some(val.to_string());
                                }
                            } else if attr.key.as_ref() == b"address"
                                && let Ok(val) = std::str::from_utf8(&attr.value)
                            {
                                current_var_addr = val.parse().ok();
                            }
                        }
                    }
                    b"type" => in_type = true,
                    b"documentation" => in_doc = true,
                    other if in_type => {
                        let ty_name = String::from_utf8_lossy(other).to_uppercase();
                        current_var_type = match ty_name.as_str() {
                            "BOOL" => Some(IecType::Bool),
                            "BYTE" | "WORD" | "INT" => Some(IecType::Int),
                            "DWORD" | "DINT" => Some(IecType::Dint),
                            "REAL" => Some(IecType::Real),
                            "TIME" => Some(IecType::Time),
                            "STRING" => Some(IecType::String),
                            _ => Some(IecType::Int),
                        };
                    }
                    _ => {}
                },
                Ok(Event::Empty(ref e)) => {
                    if in_type {
                        let ty_name = String::from_utf8_lossy(e.name().as_ref()).to_uppercase();
                        current_var_type = match ty_name.as_str() {
                            "BOOL" => Some(IecType::Bool),
                            "BYTE" | "WORD" | "INT" => Some(IecType::Int),
                            "DWORD" | "DINT" => Some(IecType::Dint),
                            "REAL" => Some(IecType::Real),
                            "TIME" => Some(IecType::Time),
                            "STRING" => Some(IecType::String),
                            _ => Some(IecType::Int),
                        };
                    }
                }
                Ok(Event::CData(ref e)) => {
                    if let Ok(text) = std::str::from_utf8(e.as_ref())
                        && let Some(ref pou_name) = current_pou_name
                    {
                        if in_body_st {
                            imported_pous.push((pou_name.clone(), LanguageKind::St, text.to_string()));
                        } else if in_body_ld {
                            imported_pous.push((pou_name.clone(), LanguageKind::Ld, text.to_string()));
                        } else if in_body_sfc {
                            imported_pous.push((pou_name.clone(), LanguageKind::Sfc, text.to_string()));
                        } else if in_body_fbd {
                            imported_pous.push((pou_name.clone(), LanguageKind::Fbd, text.to_string()));
                        }
                    }
                }
                Ok(Event::Text(ref e)) => {
                    if in_doc {
                        if let Ok(text) = e.unescape() {
                            current_var_comment = text.to_string();
                        }
                    } else if let Ok(text) = std::str::from_utf8(e.as_ref())
                        && let Some(ref pou_name) = current_pou_name
                        && in_body_st
                        && !text.trim().is_empty()
                    {
                        imported_pous.push((pou_name.clone(), LanguageKind::St, text.to_string()));
                    }
                }
                Ok(Event::End(ref e)) => match e.name().as_ref() {
                    b"pou" => current_pou_name = None,
                    b"ST" => in_body_st = false,
                    b"LD" => in_body_ld = false,
                    b"SFC" => in_body_sfc = false,
                    b"FBD" => in_body_fbd = false,
                    b"type" => in_type = false,
                    b"documentation" => in_doc = false,
                    b"variable" => {
                        if let Some(name) = current_var_name.take() {
                            let ty = current_var_type.unwrap_or(IecType::Bool);
                            let init_val = match ty {
                                IecType::Bool => IecValue::Bool(false),
                                IecType::Int => IecValue::Int(0),
                                IecType::Dint => IecValue::Dint(0),
                                IecType::Real => IecValue::Real(0.0),
                                IecType::Time => IecValue::Time(0),
                                IecType::String => IecValue::String(String::new()),
                            };
                            global_tags.push(TagEntry::new(
                                name,
                                ty,
                                current_var_addr.take(),
                                init_val,
                                current_var_comment.clone(),
                            ));
                        }
                    }
                    _ => {}
                },
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(ProjectError::Xml(format!("PLCopen XML parsing error: {e}")));
                }
                _ => {}
            }
            buf.clear();
        }

        // Scaffold new project directory
        let mut proj = Project::scaffold(target_dir, &project_name)?;
        if !global_tags.is_empty() {
            proj.global_tags.tags = global_tags;
        }

        // Write imported POUs into project
        for (pou_name, kind, content) in imported_pous {
            let rel_path = match kind {
                LanguageKind::St => format!("pous/{pou_name}.st"),
                LanguageKind::Ld => format!("pous/{pou_name}.ld.json"),
                LanguageKind::Sfc => format!("pous/{pou_name}.sfc.json"),
                LanguageKind::Fbd => format!("pous/{pou_name}.fbd.json"),
            };
            let abs_path = target_dir.join(&rel_path);
            std::fs::write(&abs_path, &content).map_err(|source| ProjectError::Io {
                path: abs_path.clone(),
                source,
            })?;

            match kind {
                LanguageKind::St => {
                    if let Ok(st) = ferrite_lang_st::StProgram::parse(&pou_name, &content) {
                        proj.pous.insert(pou_name.clone(), PouSource::St(st));
                    }
                }
                LanguageKind::Ld => {
                    if let Ok(ld) = serde_json::from_str(&content) {
                        proj.pous.insert(pou_name.clone(), PouSource::Ld(ld));
                    }
                }
                LanguageKind::Sfc => {
                    if let Ok(sfc) = serde_json::from_str(&content) {
                        proj.pous.insert(pou_name.clone(), PouSource::Sfc(sfc));
                    }
                }
                LanguageKind::Fbd => {
                    if let Ok(fbd) = serde_json::from_str(&content) {
                        proj.pous.insert(pou_name.clone(), PouSource::Fbd(fbd));
                    }
                }
            }

            if !proj.manifest.pous.iter().any(|p| p.name == pou_name) {
                proj.manifest.pous.push(PouManifestEntry {
                    name: pou_name.clone(),
                    language: kind,
                    path: rel_path,
                });
            }
        }

        proj.save(target_dir)?;
        Ok(proj)
    }
}
