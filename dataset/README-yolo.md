# Dataset for image recognition

## Tip

Tip: Use tools like LabelImg, Roboflow, CVAT, or MakeSense to annotate images and export in YOLO format automatically.
- https://github.com/wkentaro/labelme 

- label studio

      docker pull heartexlabs/label-studio:latest
      docker run -it -p 8888:8080 -v $(pwd)/dataset:/label-studio/data heartexlabs/label-studio:latest label-studio --log-level DEBUG

## Trian

yolo task=detect mode=train model=yolov8n.pt data=/path/to/data.yaml epochs=100 imgsz=640 batch=16 device=0

## File layout

- images/: Contains your screenshots (PNG/JPG).
- images/: One .txt file per image with annotations in YOLO format.

dataset/
├── images/
│   ├── train/
│   │   ├── img1.png
│   │   ├── img2.png
│   │   └── ...
│   ├── val/
│   └── test/          (optional)
└── labels/
    ├── train/
    │   ├── img1.txt
    │   ├── img2.txt
    │   └── ...
    ├── val/
    └── test/          (optional)