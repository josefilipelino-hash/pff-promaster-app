//! Remoção de dados pessoais de textos de diagnóstico (crash logs, Fase 13).
//!
//! Caminhos de ficheiros podem revelar nomes de clientes, de pastas pessoais ou do
//! utilizador ("/Users/ana/Casamentos/Silva & Costa/IMG_0001.CR3"); emails também. Um
//! crash log tem de servir para diagnosticar a aplicação sem levar nada disso.

/// Substitui caminhos absolutos (Unix, `~`, Windows, UNC) por `<caminho>` e endereços de
/// email por `<email>`. Mantém o resto do texto (mensagens de erro, ficheiros-fonte
/// relativos como `src/lib.rs:10`).
pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let prev = if i == 0 { ' ' } else { chars[i - 1] };
        let boundary = !(prev.is_alphanumeric() || prev == '_' || prev == '.' || prev == '-');
        let starts_path = boundary
            && ((c == '/'
                && chars
                    .get(i + 1)
                    .is_some_and(|n| n.is_alphanumeric() || *n == '.' || *n == '_'))
                || (c == '~' && chars.get(i + 1) == Some(&'/'))
                || (c.is_ascii_alphabetic()
                    && chars.get(i + 1) == Some(&':')
                    && matches!(chars.get(i + 2), Some('\\') | Some('/')))
                || (c == '\\' && chars.get(i + 1) == Some(&'\\')));
        if starts_path {
            // Até ao fim do caminho: aspas, vírgula, parênteses ou fim de linha. Espaços
            // fazem parte de nomes de pastas, por isso continua enquanto houver outro
            // separador mais à frente na mesma "palavra".
            let mut j = i;
            while j < chars.len()
                && !matches!(chars[j], '"' | '\'' | ',' | '(' | ')' | '\n' | '\r' | '\t' | '`')
            {
                j += 1;
            }
            // Recua espaços finais e, se a cauda depois do último separador tiver um
            // espaço seguido de texto sem separador, termina no primeiro desses espaços.
            let seg: String = chars[i..j].iter().collect();
            let cut = match seg.rfind(['/', '\\']) {
                Some(last) => {
                    let tail = &seg[last..];
                    match tail.find(": ").or_else(|| tail.find(" (")) {
                        Some(k) => last + k,
                        None => seg.trim_end().len(),
                    }
                }
                None => seg.trim_end().len(),
            };
            out.push_str("<caminho>");
            i += seg[..cut].chars().count();
            continue;
        }
        if c == '@' && i > 0 && chars.get(i + 1).is_some_and(|n| n.is_alphanumeric()) {
            // Email: apaga a parte local já escrita e salta o domínio.
            while out
                .chars()
                .last()
                .is_some_and(|p| p.is_alphanumeric() || matches!(p, '.' | '_' | '-' | '+'))
            {
                out.pop();
            }
            let mut j = i + 1;
            while j < chars.len() && (chars[j].is_alphanumeric() || matches!(chars[j], '.' | '-')) {
                j += 1;
            }
            out.push_str("<email>");
            i = j;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_paths_and_emails_but_keeps_the_message() {
        let t = "original não encontrado: /Users/ana/Casamentos/Silva & Costa/IMG_0001.CR3 (No such file)";
        assert_eq!(redact(t), "original não encontrado: <caminho> (No such file)");
        assert_eq!(
            redact(r"falhou em C:\Users\Rui\Fotos\cliente.jpg: acesso negado"),
            "falhou em <caminho>: acesso negado"
        );
        assert_eq!(redact("projeto=~/Trabalho/Joana.momento"), "projeto=<caminho>");
        assert_eq!(redact(r"\\servidor\partilha\a.jpg"), "<caminho>");
        assert_eq!(redact("contacto ana.silva+fotos@exemplo.pt ok"), "contacto <email> ok");
        // Sem dados pessoais: fica igual (fonte relativa, números, texto).
        let keep = "panicked at packages/raw-engine/src/codec.rs:42:7: índice 3 fora de 2/4";
        assert_eq!(redact(keep), keep);
    }
}
