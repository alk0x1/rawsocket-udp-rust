// Importando bibliotecas padrão necessárias para estruturas de dados, manipulação de entrada/saída, rede e operações de arquivo.
use std::collections::{HashMap, HashSet};
use std::io::{self, stdin, Write};
use std::net::UdpSocket;
use std::{env, fs, str};
use std::time::Duration;
use std::fs::File;

fn main() -> io::Result<()> {
    // Solicitando inputs do usuário
	println!("Enter the server IP address and port (e.g., '127.0.0.1:8083'):");
	let server_addr = read_input()?;
	println!("Enter the name of the file to retrieve from the server:");
	let filename = read_input()?;
    println!("Você gostaria de simular perda de pacote? (sim/não)");
    let simulate_loss = read_input()?.to_lowercase() == "sim";
    let mut loss_packets = HashSet::new();

    // Se a simulação de perda está ativada, pegue os números de sequência dos pacotes que devem ser perdidos.
    if simulate_loss {
        println!("Digite os números de sequência dos pacotes para simular a perda (separados por vírgulas, sem espaços):");
        let loss_input = read_input()?;
        loss_packets = loss_input.split(',')
            .filter_map(|num| num.parse::<u32>().ok())
            .collect();
    }
    
    // Construindo uma solicitação GET para o arquivo especificado.
    let message = format!("GET /{}", filename);
    // Ligando o socket UDP a uma porta disponível aleatória.
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    // Definindo um tempo limite de leitura de 5 segundos para o socket UDP.
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    // Enviando a solicitação GET construída para o servidor.
    send_request(&socket, &server_addr, &message)?;

    // Estrutura de dados para armazenar pacotes recebidos e números de sequência.
    let mut packets: HashMap<u32, Vec<u8>> = HashMap::new();
    let mut received_seq_numbers = HashSet::new();
    let mut expected_num_packets;

    // Loop principal para receber todos os pacotes.
    loop {
        let (new_data, new_seqs, maybe_error) = receive_response(&socket, simulate_loss, &mut loss_packets)?;
        if let Some(err) = maybe_error {
            println!("Error from server: {}", err);
            return Ok(()); // Stop processing if an error is received
        }

        for (data, &seq_number) in new_data.iter().zip(new_seqs.iter()) {
            packets.insert(seq_number, data.clone());
            received_seq_numbers.insert(seq_number);
            println!("Número de sequência recebido: {}", seq_number);
        }

        // Calcular quantos pacotes são esperados e identificar quaisquer faltantes.
        expected_num_packets = calculate_expected_number_of_packets(&received_seq_numbers);
        let missing_packets = identify_missing_packets(&received_seq_numbers, expected_num_packets);

        if missing_packets.is_empty() {
            println!("Todos os pacotes recebidos. Prosseguindo para escrita do arquivo.");
            break;  // O loop só se encerrará quando todos os pacotes forem recebidos.
        } else {
            println!("Pacotes faltando detectados: {:?}", missing_packets);
            request_retransmission(&socket, &server_addr, &missing_packets)?;
        }
    }

    // Escrevendo todos os pacotes recebidos no arquivo, em ordem.
    match write_to_file(&filename, &packets, expected_num_packets) {
        Ok(_) => println!("Arquivo '{}' salvo com sucesso.", filename),
        Err(err) => println!("Erro ao salvar o arquivo: {}", err)
    }

    Ok(())
}

// Função para ler a entrada do usuário e tratar erros.
fn read_input() -> io::Result<String> {
    let mut input = String::new();
    stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

// Função para enviar requisição para o servidor UDP.
fn send_request(socket: &UdpSocket, server_addr: &str, message: &str) -> io::Result<()> {
    socket.send_to(message.as_bytes(), server_addr)?;
    Ok(())
}

// Função para receber respostas de um servidor, que tambem pode simular perda de pacotes.
fn receive_response(socket: &UdpSocket, simulate_loss: bool, loss_packets: &mut HashSet<u32>) -> io::Result<(Vec<Vec<u8>>, HashSet<u32>, Option<String>)>  {
    let mut packets = Vec::new();
    let mut seq_numbers = HashSet::new();  // Utiliza um HashSet para garantir entradas únicas.
    let mut buf = [0; 1500];  // Buffer para os dados recebidos.
    let mut error_message: Option<String> = None;

    loop {
        match socket.recv_from(&mut buf) {
            Ok((size, _)) => {
                if size < 12 { // Tamanho mínimo para conter seq_number.
                    continue;
                }

                let seq_number = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
                if seq_number == u32::MAX { // Sinal de fim de transmissão.
                    break;
                }

                let message = std::str::from_utf8(&buf[..size]).unwrap_or("");
                if message.starts_with("ERROR") {
                    error_message = Some(message.to_owned());
                    break;
                }

                if simulate_loss && loss_packets.contains(&seq_number) {
                    println!("Pacote com número de sequência {} foi artificialmente descartado para simular perda.", seq_number);
                    loss_packets.remove(&seq_number); // Remover do conjunto para permitir a retransmissão
                    continue; // Não adicionar aos seq_numbers ou pacotes
                }

                let received_checksum = u16::from_be_bytes([buf[10], buf[11]]);
                let packet_data = &buf[12..size];
                let calculated_checksum = calculate_checksum(packet_data);
                if calculated_checksum == received_checksum {
                    packets.push(packet_data.to_vec());
                    seq_numbers.insert(seq_number);  // Armazenar número de sequência válido e não descartado
                    println!("Pacote {} recebido com checksum correto: esperado {}, obtido: {}", seq_number, calculated_checksum, received_checksum);
                } else {
                    println!("Incompatibilidade de checksum para o pacote {}: esperado {}, obtido {}", seq_number, calculated_checksum, received_checksum);
                }
            },
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                continue; // Tratar timeout
            },
            Err(e) => return Err(e), // Tratar outros erros
        }
    }
    Ok((packets, seq_numbers, error_message)) // Retornar os dados e seus números de de sequência
}

fn calculate_checksum(data: &[u8]) -> u16 {
    let sum: u32 = data
        .chunks(2)
        .fold(0, |acc, chunk| {
            let word = chunk
                .iter()
                .enumerate()
                .fold(0u16, |word_acc, (i, &byte)| word_acc | ((byte as u16) << ((1 - i) * 8)));
            acc + word as u32
        });

    let wrapped_sum = (sum & 0xFFFF) + (sum >> 16);
    let wrapped_sum = (wrapped_sum & 0xFFFF) + (wrapped_sum >> 16);
    !wrapped_sum as u16
}

// Função para calcular o número esperado de pacotes baseado no tamanho dos dados ou no último número de sequência.
fn calculate_expected_number_of_packets(received_seq_numbers: &HashSet<u32>) -> u32 {
    if received_seq_numbers.is_empty() {
        0
    } else {
        let max_seq_num = *received_seq_numbers.iter().max().unwrap();
        max_seq_num + 1
    }
}

// Função para identificar pacotes que estão faltando com base nos números de sequência recebidos e no número esperado de pacotes.
fn identify_missing_packets(received_seq_numbers: &HashSet<u32>, expected_num_packets: u32) -> Vec<u32> {
    let missing_packets = (0..expected_num_packets)
        .filter(|n| !received_seq_numbers.contains(n))
        .collect::<Vec<u32>>();

    println!("Pacotes faltantes: {:?}", missing_packets);

    missing_packets
}

// Função para solicitar retransmissão para pacotes faltantes
fn request_retransmission(socket: &UdpSocket, server_addr: &str, missing_packets: &[u32]) -> io::Result<()> {
    if missing_packets.is_empty() {
        return Ok(());
    }
    let request_string = format!("RETRANSMIT {}", missing_packets.iter().map(|num| num.to_string()).collect::<Vec<_>>().join(","));
    println!("Solicitando retransmissão para pacotes: {}", request_string);
    socket.send_to(request_string.as_bytes(), server_addr)?;
    Ok(())
}

// Função para escrever dados recebidos em um arquivo em ordem, com base nos números de sequência.
fn write_to_file(path: &str, packets: &HashMap<u32, Vec<u8>>, count: u32) -> io::Result<()> {
    let exe_path = env::current_exe()?;
    let exe_dir = exe_path.parent().ok_or(io::Error::new(io::ErrorKind::Other, "Falha ao obter diretório executável"))?;
    let files_dir = exe_dir.join("../../src/client_files");

    fs::create_dir_all(&files_dir)?;
    let file_path = files_dir.join(path);
    let mut file = File::create(&file_path)?;

    for i in 0..count {
        if let Some(data) = packets.get(&i) {
            file.write_all(data)?;
        }
    }

    Ok(())
}
