#!/usr/bin/env python3

import sqlite3
import click
import collections
import json
import cairo
import math
import os.path


@click.command()
@click.option('--output', default='.', type=click.Path(file_okay=False))
@click.option('--kind', default='tiles', type=click.Choice(('tiles', 'heights')))
@click.argument('path', type=click.Path(exists=True, dir_okay=False))
def main(path, output, kind):
    with sqlite3.connect(path) as db:
        tiles = get_tiles(db)
        grids = get_grids(db)
        segments = collections.defaultdict(list)
        for grid in grids:
            segments[grid.segment_id].append(grid)
        for segment_id, grids in segments.items():
            if kind == 'tiles':
                image = generate_tiles_image(grids=grids, tiles=tiles)
            elif kind == 'heights':
                image = generate_heights_image(grids=grids)
            image.write_to_png(os.path.join(output, f'{segment_id}.{kind}.png'))


def generate_heights_image(grids):
    max_height = max(max(v.heights) for v in grids) or 1
    min_x = min(grids, key=lambda v: v.position[0]).position[0]
    min_y = min(grids, key=lambda v: v.position[1]).position[1]
    max_x = max(grids, key=lambda v: v.position[0]).position[0]
    max_y = max(grids, key=lambda v: v.position[1]).position[1]
    width = int(math.ceil((max_x - min_x) * GRID_SIZE * TILE_SIZE))
    height = int(math.ceil((max_y - min_y) * GRID_SIZE * TILE_SIZE))
    surface = cairo.ImageSurface(cairo.FORMAT_ARGB32, width, height)
    ctx = cairo.Context(surface)
    for grid in grids:
        for x in range(GRID_SIZE):
            for y in range(GRID_SIZE):
                height = grid.heights[get_grid_tile_index(x, y)]
                color = make_heat_color(height / max_height)
                ctx.rectangle(
                    ((grid.position[0] - min_x) * GRID_SIZE + x) * TILE_SIZE,
                    ((grid.position[1] - min_y) * GRID_SIZE + y) * TILE_SIZE,
                    TILE_SIZE,
                    TILE_SIZE
                )
                ctx.set_source_rgb(*color)
                ctx.fill()
    return surface


def generate_tiles_image(grids, tiles):
    min_x = min(grids, key=lambda v: v.position[0]).position[0]
    min_y = min(grids, key=lambda v: v.position[1]).position[1]
    max_x = max(grids, key=lambda v: v.position[0]).position[0]
    max_y = max(grids, key=lambda v: v.position[1]).position[1]
    width = int(math.ceil((max_x - min_x) * GRID_SIZE * TILE_SIZE))
    height = int(math.ceil((max_y - min_y) * GRID_SIZE * TILE_SIZE))
    surface = cairo.ImageSurface(cairo.FORMAT_ARGB32, width, height)
    ctx = cairo.Context(surface)
    for grid in grids:
        for x in range(GRID_SIZE):
            for y in range(GRID_SIZE):
                tile_id = grid.tiles[get_grid_tile_index(x, y)]
                tile = tiles.get(tile_id)
                if tile is None:
                    print(f'Tile {tile_id} is not found at grid_id={grid.id} x={x} y={y}')
                color = make_rgb_color(0xFFFFFF if tile is None else tile.color)
                ctx.rectangle(
                    ((grid.position[0] - min_x) * GRID_SIZE + x) * TILE_SIZE,
                    ((grid.position[1] - min_y) * GRID_SIZE + y) * TILE_SIZE,
                    TILE_SIZE,
                    TILE_SIZE
                )
                ctx.set_source_rgb(*color)
                ctx.fill()
    return surface


def make_heat_color(heat):
    value = max(0, min(1, heat))
    if value < 0.2:
        return 0, 5 * value, 1
    if value < 0.4:
        return 0, 1, 1 - 5 * (value - 0.2)
    if value < 0.6:
        return 5 * (value - 0.4), 1, 0
    if value < 0.8:
        return 1, 1 - 5 * (value - 0.6), 0
    return 1, 0, 1 - 5 * (value - 0.8)


def get_grid_tile_index(x, y):
    return x + y * GRID_SIZE


def make_rgb_color(value):
    return get_color_component(value, 2), get_color_component(value, 1), get_color_component(value, 0)


def get_color_component(value, number):
    return ((value >> (8 * number)) & 0xFF) / 255


def get_tiles(db):
    return {v.id: v for v in (Tile(*row) for row in db.execute(GET_TILES))}


def get_grids(db):
    return [make_grid(*row) for row in db.execute(GET_GRIDS)]


def make_grid(grid_id, revision, segment_id, position_x, position_y, heights, tiles):
    return Grid(
        id=grid_id,
        revision=revision,
        segment_id=segment_id,
        position=(position_x, position_y),
        heights=json.loads(heights),
        tiles=json.loads(tiles),
    )


Tile = collections.namedtuple('Tile', ('id', 'version', 'name', 'color'))
Grid = collections.namedtuple('Grid', ('id', 'revision', 'segment_id', 'position', 'heights', 'tiles'))


GRID_SIZE = 100
TILE_SIZE = 11.0

GET_TILES = '''
    SELECT tile_id, version, name, color
      FROM tiles
     ORDER BY tile_id
'''

GET_GRIDS = '''
    SELECT grid_id, revision, segment_id, position_x, position_y, heights, tiles
      FROM grids
     ORDER BY grid_id
'''


if __name__ == '__main__':
    main()
