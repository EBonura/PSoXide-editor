"""48 triangles: faceted wrist collar, hand envelope, recessed muzzle. No grip."""
import math

def cannon_mesh():
    vertices=[];faces=[]
    for z,radius in [(-5000,2400),(-1800,3400),(12000,3000),(12000,2000)]:
        for i in range(6):
            a=math.tau*i/6
            vertices.append((round(math.cos(a)*radius),round(math.sin(a)*radius),z))
    for ring in range(3):
        for i in range(6):
            a=ring*6+i;b=ring*6+(i+1)%6;c=b+6;d=a+6
            faces.extend([(a,b,c),(a,c,d)])
    vertices.extend([(0,0,-5000),(0,0,10800)])
    for i in range(6):
        faces.append((24,(i+1)%6,i))
        faces.append((25,18+i,18+(i+1)%6))
    return vertices,faces
