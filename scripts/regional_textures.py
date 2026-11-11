#!/usr/bin/env python3
"""Original MIT periodic material tiles: R mean-normalized tint, G roughness, BA normal XY."""
from pathlib import Path
import argparse
import numpy as np
from PIL import Image

def generate(destination: Path):
    size = 512
    y, x = np.mgrid[0:size, 0:size] / size
    for index, kind in enumerate(('sand','rock','tile','glass','pavers','snow','ice')):
        rng = np.random.default_rng(8121 + index)
        grain = rng.uniform(-1, 1, (size,size))
        broad = np.sin(x*np.pi*8 + np.sin(y*np.pi*4))*np.cos(y*np.pi*6)
        seam = np.zeros_like(x)
        relief = broad*.04 + grain*.01
        rough = np.full_like(x,.9)
        if kind == 'sand':
            relief = np.sin(y*np.pi*32+np.sin(x*np.pi*4))*.045+grain*.008
        elif kind == 'rock':
            seam = (np.abs(np.sin(y*np.pi*12 + np.sin(x*np.pi*4)*1.5)) < .08)*.12
            relief = broad*.10 + grain*.025-seam
        elif kind == 'tile':
            relief = np.cos(x*np.pi*16)*.08
            seam = ((y*8)%1 < .04)*.13
            relief -= seam
            rough[:] = .78
        elif kind == 'glass':
            seam = (((x*4)%1 < .025)|((y*4)%1 < .025))*.07
            relief = -seam
            rough[:] = .18
        elif kind == 'pavers':
            row = np.floor(y*8)
            seam = (((x*8 + (row%2)*.5)%1 < .05)|((y*8)%1 < .05))*.10
            relief = grain*.008-seam
            rough[:] = .86
        elif kind == 'snow':
            relief = broad*.018+grain*.012
            rough[:] = .96
        elif kind == 'ice':
            seam = (np.abs(np.sin(x*np.pi*6+np.sin(y*np.pi*4)*1.5)) < .025)*.04
            relief = broad*.012-seam
            rough[:] = .26
        light = .65 + relief*.25 - seam*.35
        light += .65-light.mean()
        dx = (np.roll(relief,-1,1)-np.roll(relief,1,1))*2
        dy = (np.roll(relief,-1,0)-np.roll(relief,1,0))*2
        pixels = np.stack((light,rough+grain*.02,.5+dx*.5,.5+dy*.5),axis=-1)
        image = Image.fromarray(np.rint(np.clip(pixels,0,1)*255).astype('uint8'))
        for tier in (128,256,512):
            folder = destination if tier == 256 else destination/str(tier)
            folder.mkdir(parents=True,exist_ok=True)
            image.resize((tier,tier),Image.Resampling.LANCZOS).save(folder/(kind+'.png'),optimize=True)

if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('destination',type=Path)
    generate(parser.parse_args().destination)
