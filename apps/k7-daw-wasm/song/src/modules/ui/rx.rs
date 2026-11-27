// rx.rs — the few regular expressions the panel rules are written in, kept as their JavaScript source so a pattern reads as it always did.
// A reader would get wrong that this is no general engine: literals, `.`, `[abc]`, `\b \s \d \w`, `^ $`, groups, `|` and `? * +`, searched anywhere, backtracking.

#[derive(Debug)]
enum Node {
    Char(char),
    Any,
    Space,
    Digit,
    Word,
    Boundary,
    Class(Vec<char>, bool),
    Start,
    End,
    Group(Vec<Vec<Node>>),
    Repeat(Box<Node>, usize, usize),
}

/// A compiled pattern.
#[derive(Debug)]
pub struct Rx {
    alts: Vec<Vec<Node>>,
    fold: bool,
}

fn parse_alts(src: &[char], at: &mut usize) -> Vec<Vec<Node>> {
    let mut done: Vec<Vec<Node>> = Vec::new();
    let mut cur: Vec<Node> = Vec::new();
    while let Some(&c) = src.get(*at) {
        *at += 1;
        let node = match c {
            ')' => break,
            '|' => {
                done.push(std::mem::take(&mut cur));
                continue;
            }
            '(' => Node::Group(parse_alts(src, at)),
            '[' => {
                let negate = src.get(*at) == Some(&'^');
                *at += usize::from(negate);
                let mut set = Vec::new();
                while let Some(&m) = src.get(*at) {
                    *at += 1;
                    if m == ']' {
                        break;
                    }
                    set.push(m);
                }
                Node::Class(set, negate)
            }
            '.' => Node::Any,
            '^' => Node::Start,
            '$' => Node::End,
            '\\' => {
                let e = src.get(*at).copied().unwrap_or('\\');
                *at += 1;
                match e {
                    'b' => Node::Boundary,
                    's' => Node::Space,
                    'd' => Node::Digit,
                    'w' => Node::Word,
                    other => Node::Char(other),
                }
            }
            '?' | '*' | '+' => {
                let Some(last) = cur.pop() else { continue };
                let (lo, hi) = match c {
                    '?' => (0, 1),
                    '*' => (0, usize::MAX),
                    _ => (1, usize::MAX),
                };
                Node::Repeat(Box::new(last), lo, hi)
            }
            other => Node::Char(other),
        };
        cur.push(node);
    }
    done.push(cur);
    done
}

const fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

impl Rx {
    /// A pattern from its JavaScript source; `fold` is the `i` flag.
    pub fn new(source: &str, fold: bool) -> Rx {
        let chars: Vec<char> = source.chars().collect();
        let mut at = 0;
        let alts = parse_alts(&chars, &mut at);
        Rx { alts, fold }
    }

    /// Whether the pattern matches anywhere in `text`.
    pub fn test(&self, text: &str) -> bool {
        let s: Vec<char> = text.chars().collect();
        (0..=s.len()).any(|start| self.alts.iter().any(|alt| self.seq(alt, &s, start, &mut |_| true)))
    }

    fn eq(&self, a: char, b: char) -> bool {
        a == b || (self.fold && a.to_lowercase().eq(b.to_lowercase()))
    }

    fn seq(&self, nodes: &[Node], s: &[char], pos: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
        let Some((first, rest)) = nodes.split_first() else {
            return k(pos);
        };
        self.one(first, s, pos, &mut |p| self.seq(rest, s, p, k))
    }

    fn one(&self, node: &Node, s: &[char], pos: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
        let at = s.get(pos).copied();
        match node {
            Node::Char(c) => at.is_some_and(|a| self.eq(a, *c)) && k(pos + 1),
            Node::Any => at.is_some_and(|a| a != '\n') && k(pos + 1),
            Node::Space => at.is_some_and(|a| a.is_whitespace() || a == '\u{feff}') && k(pos + 1),
            Node::Digit => at.is_some_and(|a| a.is_ascii_digit()) && k(pos + 1),
            Node::Word => at.is_some_and(is_word) && k(pos + 1),
            Node::Class(set, negate) => at.is_some_and(|a| set.iter().any(|c| self.eq(a, *c)) != *negate) && k(pos + 1),
            Node::Boundary => {
                let before = pos > 0 && is_word(s[pos - 1]);
                let after = at.is_some_and(is_word);
                before != after && k(pos)
            }
            Node::Start => pos == 0 && k(pos),
            Node::End => pos == s.len() && k(pos),
            Node::Group(alts) => alts.iter().any(|alt| self.seq(alt, s, pos, k)),
            Node::Repeat(inner, lo, hi) => self.repeat(inner, *lo, *hi, 0, s, pos, k),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn repeat(
        &self,
        inner: &Node,
        lo: usize,
        hi: usize,
        done: usize,
        s: &[char],
        pos: usize,
        k: &mut dyn FnMut(usize) -> bool,
    ) -> bool {
        // Greedy: one more first, then the rest; an empty step ends the loop.
        if done < hi
            && self.one(inner, s, pos, &mut |p| {
                p != pos && self.repeat(inner, lo, hi, done + 1, s, p, k)
            })
        {
            return true;
        }
        done >= lo && k(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::Rx;

    fn yes(p: &str, t: &str) -> bool {
        Rx::new(p, false).test(t)
    }

    #[test]
    fn matches_what_javascript_does() {
        assert!(yes(r"\bhpf?\b", "an hp"));
        assert!(yes(r"\bhpf?\b", "hpf cut"));
        assert!(!yes(r"\bhpf?\b", "hpfx"));
        assert!(yes("^@base$", "@base"));
        assert!(!yes("^@base$", "@base2"));
        assert!(yes(r"pre.?delay", "predelay"));
        assert!(yes(r"pre.?delay", "pre-delay"));
        assert!(yes(r"\bnote (on|off)", "a note off"));
        assert!(!yes(r"\bnote (on|off)", "a note in"));
        assert!(yes(
            r"^(low|lo)(\s*-?\s*(shelf|peak))?(\s+(gain|level))?(\s*\d+)?$",
            "low shelf gain 2"
        ));
        assert!(Rx::new(r"\bdry\b", true).test("Dry/Wet"));
        assert!(Rx::new("déplac", true).test("DÉPLACE"));
    }

    #[test]
    fn a_boundary_needs_a_word_on_one_side() {
        assert!(yes(r"\bsub\b", "sub osc"));
        assert!(!yes(r"\bsub\b", "subtle"));
        assert!(yes(r"\bctrl", "midi ctrl 3"));
    }
}
