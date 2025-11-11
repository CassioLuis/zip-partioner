use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
// use rayon::prelude::*;
use rfd::FileDialog;
use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::path::PathBuf;
use zip::{read::ZipArchive, write::FileOptions, ZipWriter};

/// Representa um XML extraído (nome + conteúdo)
struct XmlFile {
    name: String,
    data: Vec<u8>,
}

fn main() -> Result<()> {
    println!("=== ZIP Partitioner ===");

    // Seleciona arquivo ZIP de entrada
    let input_path = FileDialog::new()
        .add_filter("Arquivos ZIP", &["zip"])
        .set_title("Selecione o arquivo ZIP de entrada")
        .pick_file()
        .context("Nenhum arquivo ZIP selecionado")?;

    // Seleciona pasta de saída
    let output_dir = FileDialog::new()
        .set_title("Selecione o diretório de destino")
        .pick_folder()
        .context("Nenhum diretório de destino selecionado")?;

    // Solicita quantidade máxima de XMLs por parte
    println!("Quantos XMLs por parte?");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let max_per_part: usize = input.trim().parse().context("Número inválido")?;

    println!("Lendo e extraindo XMLs (em memória)...");

    // Lê o arquivo ZIP original em memória
    let mut buf = Vec::new();
    File::open(&input_path)?.read_to_end(&mut buf)?;
    let cursor = Cursor::new(buf);

    // Extrai recursivamente todos os XMLs
    let xmls = extract_all_xmls_from_zip(cursor)?;

    println!("Encontrados: {} XMLs", xmls.len());
    if xmls.is_empty() {
        println!("Nenhum arquivo XML encontrado.");
        return Ok(());
    }

    // Divide em partes
    let total_parts = (xmls.len() + max_per_part - 1) / max_per_part;
    println!("Gerando {} partes...", total_parts);

    let pb = ProgressBar::new(total_parts as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} partes")
            .unwrap(),
    );

    for (i, chunk) in xmls.chunks(max_per_part).enumerate() {
        let zip_name = format!("parte_{}.zip", i + 1);
        let zip_path = output_dir.join(&zip_name);

        create_zip_in_memory(chunk, &zip_path)?;
        pb.inc(1);
        println!("-> {} ({} XMLs)", zip_name, chunk.len());
    }

    pb.finish_and_clear();
    println!("Concluído com sucesso!");
    Ok(())
}

/// Extrai todos os XMLs de um ZIP (recursivamente) em memória.
fn extract_all_xmls_from_zip<R: Read + std::io::Seek>(reader: R) -> Result<Vec<XmlFile>> {
    let mut zip = ZipArchive::new(reader)?;
    let mut xmls = Vec::new();

    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        let name = file.name().to_string();

        if file.is_dir() {
            continue;
        }

        // Lê conteúdo do arquivo atual em memória
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;

        // Se for ZIP dentro do ZIP → processa recursivamente
        if name.ends_with(".zip") {
            let cursor = Cursor::new(buf);
            let inner_xmls = extract_all_xmls_from_zip(cursor)?;
            xmls.extend(inner_xmls);
        } else if name.to_lowercase().ends_with(".xml") {
            xmls.push(XmlFile { name, data: buf });
        }
    }

    Ok(xmls)
}

/// Cria um ZIP em memória com os XMLs fornecidos e salva no disco somente no final.
fn create_zip_in_memory(xmls: &[XmlFile], output_path: &PathBuf) -> Result<()> {
    let mut zip_buf = Cursor::new(Vec::new());
    {
        let mut zip_writer = ZipWriter::new(&mut zip_buf);
        let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        for xml in xmls {
            let filename = std::path::Path::new(&xml.name)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("arquivo.xml");

            zip_writer.start_file(filename, options)?;
            zip_writer.write_all(&xml.data)?;
        }

        zip_writer.finish()?;
    }

    // Grava o resultado final (ZIP completo) no diretório de saída
    let mut file = File::create(output_path)?;
    file.write_all(&zip_buf.into_inner())?;
    Ok(())
}
