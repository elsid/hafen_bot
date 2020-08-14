use crate::bot::math::as_score;
use crate::bot::vec2::{Vec2f, Vec2i};

pub fn make_adjacent_tiles_clusters(tile_poses: &Vec<Vec2i>) -> Vec<Vec<Vec2i>> {
    let mut clusters: Vec<Vec<Vec2i>> = Vec::new();
    for &tile_pos in tile_poses.iter() {
        let mut found = false;
        for cluster in clusters.iter_mut() {
            if cluster.iter().find(|&&v| is_adjacent(v, tile_pos)).is_some() {
                cluster.push(tile_pos);
                found = true;
                break;
            }
        }
        if !found {
            clusters.push(vec![tile_pos]);
        }
    }
    clusters
}

pub fn get_cluster_median(cluster: &Vec<Vec2i>) -> Option<Vec2i> {
    get_cluster_mean(&cluster).and_then(|mean| {
        cluster.iter().min_by_key(|v| as_score(v.center().distance(mean))).map(|v| *v)
    })
}

pub fn get_cluster_mean(cluster: &Vec<Vec2i>) -> Option<Vec2f> {
    cluster.first().map(|first| {
        cluster.iter().fold(first.center(), |r, v| r + v.center()) / cluster.len() as f64
    })
}

pub fn is_adjacent(lhs: Vec2i, rhs: Vec2i) -> bool {
    lhs.x() + 1 == rhs.x() || lhs.x() == rhs.x() + 1
        || lhs.y() + 1 == rhs.y() || lhs.y() == rhs.y() + 1
}
