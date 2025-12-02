use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use rfd::FileDialog;
use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::path::PathBuf;
use zip::{read::ZipArchive, write::FileOptions, ZipWriter};
use std::sync::Arc;
use std::time::Instant;
/// Representa um XML extraído (nome + conteúdo)
struct XmlFile {
	name: String,
	data: Vec<u8>,
}

fn main() -> Result<()> {
    println!("=== ZIP Partitioner ===");

    // Seleciona arquivo ZIP de entrada
    let input_paths = FileDialog::new()
			.add_filter("Arquivos ZIP", &["zip"])
			.set_title("Selecione um ou mais arquivos ZIP de entrada")
			.pick_files()
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

    // Processa todos os ZIPs em paralelo
    let results: Vec<Vec<XmlFile>> = input_paths
			.par_iter()
			.map(|input_path| {
				println!("Processando ZIP: {}", input_path.display());

				let mut buf = Vec::new();
				if let Err(err) = File::open(input_path).and_then(|mut f| f.read_to_end(&mut buf)) {
					eprintln!("Erro lendo {}: {}", input_path.display(), err);
					return Vec::new();
				}

				let cursor = Cursor::new(buf);

				match extract_all_xmls_from_zip(cursor) {
					Ok(xmls) => xmls,
					Err(err) => {
						eprintln!("Erro ao processar {}: {}", input_path.display(), err);
						Vec::new()
					}
				}
			})
			.collect();

    // Junta tudo em um único vetor geral
    let mut all_xmls = Vec::new();

    for mut xml_group in results {
			all_xmls.append(&mut xml_group);
    }

    println!("Total de XMLs: {}", all_xmls.len());
    if all_xmls.is_empty() {
			println!("Nenhum XML encontrado em nenhum ZIP!");
			return Ok(());
    }

    // Divide em partes
    let total_parts = (all_xmls.len() + max_per_part - 1) / max_per_part;
    println!("Gerando {} partes...", total_parts);

		// cria o timer total
		let total_timer = Instant::now();

    let pb = Arc::new(ProgressBar::new(total_parts as u64));
    pb.set_style(
			ProgressStyle::default_bar()
				.template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} partes ({percent}% {msg})\n")
				.unwrap(),
    );

    // Processa cada parte em paralelo
    all_xmls
			.par_chunks(max_per_part)
			.enumerate()
			.for_each(|(i, chunk)| {
				let zip_name = format!("parte_{}.zip", i + 1);
				let zip_path = output_dir.join(&zip_name);

				if let Err(err) = create_zip_in_memory(chunk, &zip_path) {
					pb.println(format!("ERRO na {}: {}", zip_name, err));
					return;
				}

				pb.inc(1);
			});

		let total_elapsed = total_timer.elapsed();
		pb.finish_with_message(format!("Tempo total: {:.4}", total_elapsed.as_secs_f64()));

    println!("\n\x1b[1;92m🎉 Concluído com sucesso! 🎉\x1b[0m");
    println!("\nPressione ENTER para fechar...");
    let mut dummy = String::new();
    std::io::stdin().read_line(&mut dummy).ok();

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
		if name.to_lowercase().ends_with(".zip") {
			// Cria um cursor em memória para o zip interno
			let cursor = Cursor::new(buf);
			// Chamada recursiva profunda
			match extract_all_xmls_from_zip(cursor) {
				Ok(mut inner_xmls) => xmls.append(&mut inner_xmls),
				Err(e) => eprintln!("Aviso: falha ao ler ZIP interno '{}': {}", name, e),
			}
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
