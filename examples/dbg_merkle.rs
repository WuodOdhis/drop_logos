use airdrop_core::merkle::*;
fn main() {
    let d = compute_default_hashes(8);
    let tree = SparseMerkleTree::new(8);
    println!("root: {:?}", tree.root());
    println!("defaults[0]: {:?}", d[0]);
    let path = tree.path(0);
    println!("path len {}: {:?}", path.len(), path);
    let v = SparseMerkleTree::verify_inclusion(&tree.root(), &ZERO, &path, 0);
    println!("verify: {}", v);
    // manual fold
    let mut h = ZERO;
    for (i, sib) in path.iter().enumerate() {
        h = hash_pair(&h, sib);
        println!("fold {}: {:?}", i, h);
    }
    println!("folded: {:?} eq defaults[0]: {}", h == d[0], hex::encode(h));
}
