fn main() {
    let leaves = vec![
        "a".to_string(),
        "b".to_string(),
        "c".to_string(),
        "d".to_string(),
        "e".to_string(),
        "f".to_string(),
        "g".to_string(),
        "h".to_string(),
        "i".to_string(),
        "j".to_string(),
        "k".to_string(),
        "l".to_string(),
        "m".to_string(),
        "n".to_string(),
        "o".to_string(),
        "p".to_string(),
    ];
    let leaves = vec![
        "a".to_string(),
        "b".to_string(),
        "c".to_string(),
        "d".to_string(),
        "e".to_string(),
    ];
    let tree = build_binary_tree(leaves.clone());
    println!("{:?}", tree);
    print_parents(leaves.len(), tree.clone(), 4);
}

fn print_parents(leaves_len: usize, tree: Vec<String>, leaf_index: usize) {
    println!("{}", tree[leaf_index]);
    let mut level_offset = 0;
    let mut level_len = leaves_len;
    let mut leaf_index = leaf_index;
    let mut current_index = leaf_index;
    println!("{} {} {}", level_len, level_offset, current_index);
    while level_offset + level_len <= tree.len() {
        // println!("{}", tree[parent_index]);
        // println!("{} {}", tree[current_index], tree[current_index + 1]);
        let level = &tree[level_offset..(level_offset + level_len)];
        println!("{:?}", level);
        if current_index % 2 == 0 {
            let right_index = if current_index + 1 < level_len {
                leaf_index + 1
            } else {
                leaf_index
            };

            println!("{} {}", tree[current_index], tree[right_index]);
        } else {
            println!("{} {}", tree[current_index - 1], tree[current_index]);
        }

        leaf_index /= 2;
        level_offset += level_len;
        current_index = level_offset + leaf_index / 2 + leaf_index % 2;
        current_index = leaf_index;
        level_len = (level_len + 1) / 2;
        println!("{} {} {}", level_len, level_offset, current_index);
    }
    println!("{}", tree[current_index]);
}

fn build_binary_tree(leaves: Vec<String>) -> Vec<String> {
    let leaf_count = leaves.len();
    let mut nodes = Vec::with_capacity(leaf_count * 2);
    nodes.extend_from_slice(leaves.as_slice());

    let mut prev_level_len = leaf_count;
    let mut prev_offset = 0;
    let mut current_offset = leaf_count;

    while prev_level_len > 1 {
        let current_level_len = (prev_level_len + 1) / 2;

        for i in 0..current_level_len {
            let prev_index = i * 2;
            let left = nodes[prev_offset + prev_index].clone();
            let right = if prev_index + 1 < prev_level_len {
                nodes[prev_offset + prev_index + 1].clone()
            } else {
                nodes[prev_offset + prev_index].clone()
            };
            let parent = format!("{}{}", left, right);
            nodes.push(parent);
        }
        prev_level_len = current_level_len;
        prev_offset = current_offset;
        current_offset += current_level_len;
    }
    nodes
}
