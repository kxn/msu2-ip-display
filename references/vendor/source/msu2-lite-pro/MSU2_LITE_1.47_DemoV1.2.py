# -*- coding: UTF-8 -*-
import serial#引入串口库（需要额外安装）
import serial.tools.list_ports
import time#引入延时库
import threading#引入定时回调库
import psutil #引入psutil获取设备信息（需要额外安装）
import os#用于读取文件
from datetime import datetime#用于获取当前时间
import tkinter as tk
from tkinter import *#引入UI库
import tkinter.filedialog#用于获取文件路径
from PIL import Image#引入PIL库进行图像处理
import sys#用于关闭程序
import dxcam#用于截屏
import cv2#用于缩小图像
import numpy as np#用于RGB656转换
from ctypes import *
#from ctypes import c_uint16, POINTER, cast, create_string_buffe

#lib = WinDLL('D:\Share\MSU2_1.47LCD\compaction.dll')  # Windows(用于高效压缩数据）
#lib2 = WinDLL('D:\Share\MSU2_1.47LCD\OpenHardwareMonitorLib.dll')  # Windows(用于高效压缩数据）

lib = WinDLL('compaction.dll')  # Windows(用于高效压缩数据）
#lib2 = WinDLL('OpenHardwareMonitorLib.dll') # 用于获取系统信息

#import clr
#clr.AddReference(r'C:\Users\Administrator\Desktop\MSU2_PRO\MSU2_Pro\OpenHardwareMonitorLib.dll')#绝对路径
#clr.AddReference('OpenHardwareMonitorLib')#相对路径
#from OpenHardwareMonitor import Hardware#导入Hardware模块
#computer = Hardware.Computer()# 创建计算机硬件监控对象
#computer.MainboardEnabled = True# 启用主板监控
#computer.CPUEnabled = True# 启用CPU监控
#computer.GPUEnabled = True# 启用显卡监控
#computer.HDDEnabled = True# 启用硬盘监控
#computer.RAMEnabled = True# 启用内存监控
#computer.FanControllerEnabled = True# 启用风扇控制器监控
#computer.Open()# 初始化硬件监控系统

#for hardware in computer.Hardware:# 遍历所有已启用的硬件设备
#    print(f"Hardware: {hardware.Name}")
#    hardware.Update()# 更新当前硬件的传感器数据
#    for sensor in hardware.Sensors:# 遍历当前硬件的所有传感器
#        print(f"  {sensor.Name}: {sensor.Value} {sensor.SensorType}")
#    for subhardware in hardware.SubHardware:#遍历当前硬件的子硬件（如多GPU系统中的副显卡）
#        print(f"  Subhardware: {subhardware.Name}")
#        subhardware.Update()# 更新子硬件的传感器数据
#        for sensor in subhardware.Sensors:# 遍历子硬件的所有传感器
#            print(f"    {sensor.Name}: {sensor.Value} {sensor.SensorType}")
#自定义系统
#print("电脑型号："+computer.Hardware[0].Name)
#print("处理器型号："+computer.Hardware[1].Name)

#while(1):
#    time.sleep(1)
#    computer.Hardware[1].Update()
#    for sensor in computer.Hardware[1].Sensors:
#        if(sensor.Name=="CPU Package"):
#            if(int(sensor.SensorType)==2):#2对应温度
#                print("处理器温度："+str(sensor.Value))
#print("硬盘型号："+computer.Hardware[3].Name)

#time.sleep(1000)
#computer.Close()

lib.MSN_compaction.argtypes = [
    POINTER(c_uint16),    # unsigned short*
    POINTER(c_uint8),     # unsigned char*
    c_uint32              # unsigned int
]

lib.MSN_compaction.restype = c_uint32

def py_compaction(rgb565_data, out_data):
    input_arr = (c_uint16 * len(rgb565_data))(*rgb565_data)
    output_arr = (c_uint8 * len(out_data))(*out_data)
    result_len = lib.MSN_compaction(
        input_arr,         
        output_arr,        
        len(rgb565_data)   
    )
    return bytes(output_arr[:result_len])


class MSN_Device:#定义一个结构体
    def __init__(self,com,version):
        self.com=com #登记串口位置
        self.version=version#登记MSN版本
        self.name='MSN'  #登记设备名称
        self.baud_rate=19200  #登记波特率
My_MSN_Device=[]#创建一个空的结构体数组

class MSN_Data:#定义一个结构体
    def __init__(self,name,unit,family,data):
        self.name=name 
        self.unit=unit
        self.family=family 
        self.data=data  

My_MSN_Data=[]#创建一个空的结构体数组

#颜色对应的RGB565编码
RED=0xf800
GREEN=0x07e0
BLUE=0x001f
WHITE=0xffff
BLACK=0x0000
YELLOW=0xFFE0
GRAY0=0xEF7D
GRAY1=0x8410
GRAY2=0x4208

hex_code=b''

G_screnn0=bytearray()#空数组
G_screnn1=bytearray()#空数组
Img_data_use=bytearray()#空数组
G_screnn0_OK=0
G_screnn1_OK=0
size_USE_X1=0
size_USE_Y1=0


#参数定义
Show_W = 500#显示宽度
Show_H = 350#画布高度

LCD_X=160;
LCD_Y=80;
#按键功能定义
def Get_Photo_Path1():#获取文件路径
    global photo_path1,Label3
    photo_path1=tk.filedialog.askopenfilename(title="选择文件",filetypes=[('Image file','*.jpg'),('Image file','*.jpeg'),('Image file','*.png'),('Image file','*.bmp')])
    Label3.config(text=photo_path1[-20:])
    
    #photo_path1=photo_path1[:-4]
    #print(photo_path1)
    
    
def Get_Photo_Path2():#获取文件路径
    global photo_path2,Label4
    photo_path2=tk.filedialog.askopenfilename(title="选择文件",filetypes=[('Bin file','*.bin')])
    Label4.config(text=photo_path2[-20:])
    photo_path2=photo_path2[:-4]
    #print(photo_path2)
    
def Get_Photo_Path3():#获取文件路径
    global photo_path3,Label5#支持JPG、PNG、BMP图像格式
    photo_path3=tk.filedialog.askopenfilename(title="选择文件",filetypes=[('Image file','*.jpg'),('Image file','*.jpeg'),('Image file','*.png'),('Image file','*.bmp')])
    Label5.config(text=photo_path3[-20:])
    
    #photo_path3=photo_path3[:-4]
    #print(photo_path3)
    
def Get_Photo_Path4():#获取文件路径
    global photo_path4,Label6
    photo_path4=tk.filedialog.askopenfilename(title="选择文件",filetypes=[('Image file','*.jpg'),('Image file','*.jpeg'),('Image file','*.png'),('Image file','*.bmp')])
    Label6.config(text=photo_path4[-20:])
    
    
    #photo_path4=photo_path4[:-4]
    #print(photo_path4)
    
    
def Writet_Photo_Path1():#写入文件
    global photo_path1,write_path1,Text1,Img_data_use
    if write_path1==0:#确保上次执行写入完毕
        Text1.delete(1.0,END)#清除文本框
        Text1.insert(END,'图像格式转换...\n')#在文本框开始位置插入“内容一”
        im1=Image.open(photo_path1)
        
        if im1.width*LCD_Y>=(im1.height*LCD_X):#图片长宽比例超过2:1
            im2=im1.resize((int(LCD_Y*im1.width/im1.height),LCD_Y))
            Img_m=int(im2.width/2)
            box=((Img_m-int(LCD_X/2),0,Img_m+int(LCD_X/2),LCD_Y))#定义需要裁剪的空间
            im2=im2.crop(box)
        else:
            im2=im1.resize((LCD_X,int(LCD_X*im1.height/im1.width)))
            Img_m=int(im2.height/2)
            box=((0,Img_m-int(LCD_Y/2),LCD_X,Img_m+int(LCD_Y/2)))#定义需要裁剪的空间
            im2=im2.crop(box)
        im2=im2.convert('RGB')#转换为RGB格式
        Img_data_use=bytearray()#空数组
        for y in range(0,LCD_Y):#逐字解析编码
            for x in range(0,LCD_X):#逐字解析编码
                r,g,b=im2.getpixel((x,y))
                Img_data_use.append(((r>>3)<<3)|(g>>5))
                Img_data_use.append((((g%32)>>2)<<5)|(b>>3))
        write_path1=1
        
        
    
def Writet_Photo_Path2():#写入文件
    global photo_path2,write_path2,Text1
    if write_path2==0:#确保上次执行写入完毕
        write_path2=1
        Text1.delete(1.0,END)#清除文本框
        Text1.insert(END,'准备烧写Flash固件,需要两分钟\n')#在文本框开始位置插入“内容一”
        
        
def Writet_Photo_Path3():#写入文件
    global photo_path3,write_path3,Text1,Img_data_use
    if write_path3==0:#确保上次执行写入完毕
        Text1.delete(1.0,END)#清除文本框
        Text1.insert(END,'图像格式转换...\n')#在文本框开始位置插入“内容一”
        im1=Image.open(photo_path3)  
        if im1.width*LCD_Y>=(im1.height*LCD_X):#图片长宽比例超过2:1
            im2=im1.resize((int(LCD_Y*im1.width/im1.height),LCD_Y))
            Img_m=int(im2.width/2)
            box=((Img_m-int(LCD_X/2),0,Img_m+int(LCD_X/2),LCD_Y))#定义需要裁剪的空间
            im2=im2.crop(box)
        else:
            im2=im1.resize((LCD_X,int(LCD_X*im1.height/im1.width)))
            Img_m=int(im2.height/2)
            box=((0,Img_m-int(LCD_Y/2),LCD_X,Img_m+int(LCD_Y/2)))#定义需要裁剪的空间
            im2=im2.crop(box)
        im2=im2.convert('RGB')#转换为RGB格式
        Img_data_use=bytearray()#空数组
        for y in range(0,LCD_Y):#逐字解析编码
            for x in range(0,LCD_X):#逐字解析编码
                r,g,b=im2.getpixel((x,y))
                Img_data_use.append(((r>>3)<<3)|(g>>5))
                Img_data_use.append((((g%32)>>2)<<5)|(b>>3))
        write_path3=1
        
        
        
        
        
        
        
        
        
        
        
        
        
        
        
        
        #print(img_use)
        
        #im2.show()
        #Text1.insert(END,'准备烧写背景图像...\n')#在文本框开始位置插入“内容一”

        
        
        
    
def Writet_Photo_Path4():#写入文件
    global photo_path4,write_path4,Text1,Img_data_use
    if write_path4==0:#确保上次执行写入完毕
        Text1.delete(1.0,END)#清除文本框
        Text1.insert(END,'动图格式转换中...')#在文本框开始位置插入“内容一”
        time.sleep(0.1)
        Path_use=photo_path4
        if Path_use[-4]=='.':#
            write_path4=Path_use[-4:]
            Path_use=Path_use[:-5]
            
        elif Path_use[-5]=='.':
            write_path4=Path_use[-5:]
            Path_use=Path_use[:-6]
        else:
            Text1.insert(END,'动图名称不符合要求！\n')#在文本框开始位置插入“内容一”
        Img_data_use=bytearray()
        u_time=time.time()
        for i in range(0,64):#依次转换64张图片
            Read_M_u8(123)#保持通信--必要
            im1=Image.open(Path_use+str(i)+write_path4)
            if im1.width*LCD_Y>=(im1.height*LCD_X):#图片长宽比例超过2:1
                im2=im1.resize((int(LCD_Y*im1.width/im1.height),LCD_Y))
                Img_m=int(im2.width/2)
                box=((Img_m-int(LCD_X/2),0,Img_m+int(LCD_X/2),LCD_Y))#定义需要裁剪的空间
                im2=im2.crop(box)
            else:
                im2=im1.resize((LCD_X,int(LCD_X*im1.height/im1.width)))
                Img_m=int(im2.height/2)
                box=((0,Img_m-int(LCD_Y/2),LCD_X,Img_m+int(LCD_Y/2)))#定义需要裁剪的空间
                im2=im2.crop(box)
            im2=im2.convert('RGB')#转换为RGB格式
            
            for y in range(0,LCD_Y):#逐字解析编码
                for x in range(0,LCD_X):#逐字解析编码
                    r,g,b=im2.getpixel((x,y))
                    Img_data_use.append(((r>>3)<<3)|(g>>5))
                    Img_data_use.append((((g%32)>>2)<<5)|(b>>3))
        u_time=time.time()-u_time
        u_time=int(u_time*1000)
        Text1.insert(END,'耗时'+str(u_time)+'ms,烧录预计需要80秒\n')#在文本框开始位置插入“内容一”
        write_path4=1
        
        
        
def Writet_Photo_Path_FB4():#写入文件(进行二值化,96张图片)
    global photo_path4,write_path4,Text1,Img_data_use
    if write_path4==0:#确保上次执行写入完毕
        Text1.delete(1.0,END)#清除文本框
        Text1.insert(END,'动图格式转换中...\n')#在文本框开始位置插入“内容一”
        time.sleep(0.1)
        Path_use=photo_path4
        if Path_use[-4]=='.':#
            write_path4=Path_use[-4:]
            Path_use=Path_use[:-5]
            
        elif Path_use[-5]=='.':
            write_path4=Path_use[-5:]
            Path_use=Path_use[:-6]
        else:
            Text1.insert(END,'动图名称不符合要求！\n')#在文本框开始位置插入“内容一”
        Img_data_use=bytearray()
        u_time=time.time()
        
        for i in range(0,96):#依次转换64张图片
            Read_M_u8(123)#保持通信--必要
            im1=Image.open(Path_use+str(i)+write_path4)
            if im1.width*LCD_Y>=(im1.height*LCD_X):#图片长宽比例超过2:1
                im2=im1.resize((int(LCD_Y*im1.width/im1.height),LCD_Y))
                Img_m=int(im2.width/2)
                box=((Img_m-int(LCD_X/2),0,Img_m+int(LCD_X/2),LCD_Y))#定义需要裁剪的空间
                im2=im2.crop(box)
            else:
                im2=im1.resize((LCD_X,int(LCD_X*im1.height/im1.width)))
                Img_m=int(im2.height/2)
                box=((0,Img_m-int(LCD_Y/2),LCD_X,Img_m+int(LCD_Y/2)))#定义需要裁剪的空间
                im2=im2.crop(box)
            im2=im2.convert('RGB')#转换为RGB格式
            
            for y in range(0,LCD_Y):#逐字解析编码
                for x in range(0,LCD_X):#逐字解析编码
                    r,g,b=im2.getpixel((x,y))
                    Img_data_use.append(((r>>3)<<3)|(g>>5))
                    Img_data_use.append((((g%32)>>2)<<5)|(b>>3))
        u_time=time.time()-u_time
        u_time=int(u_time*1000)
        Text1.insert(END,'转换完成,耗时'+str(u_time)+'ms\n')#在文本框开始位置插入“内容一”
        write_path4=1
        
def Page_UP():#上一页
    global State_change,State_machine
    State_machine=State_machine+1
    State_change=1
    if State_machine>5:
        State_machine=0
    

def Page_Down():#下一页
    global State_change,State_machine
    State_machine=State_machine-1
    State_change=1
    if State_machine<0:
        State_machine=5
        
def LCD_Change():#切换显示方向
    global LCD_Change_use
    LCD_Change_use=LCD_Change_use+1
    if LCD_Change_use>1:#限制切换模式
        LCD_Change_use=0




async def SER_Write_S(Data_U0):
    global Device_State
    ser.write(Data_U0)
    await ser.drain()  # 确保数据完全发
    
    #try:
    #    if(False == ser.is_open):
    #        Device_State=0#恢复到未连接状态
    #    ser.write(Data_U0)
    #    await ser.drain()  # 确保数据完全发
    #except:#出现异常
    #    Device_State=0
    #    ser.close()#将串口关闭，防止下次无法打开
        


def SER_Write(Data_U0):
    global Device_State
    #print('发送数据ing');
    try:#尝试发出指令,有两种无法正确发送命令的情况：1.设备被移除,发送出错；2.设备处于MSN连接状态，对于电脑发送的指令响应迟缓
        #进行超时检测
        #u_time=time.time()
        if(False == ser.is_open):
            Device_State=0#恢复到未连接状态
        
        ser.write(Data_U0)
        
        #print(Data_U0)
        #u_time=time.time()-u_time
        #if u_time>2:
            #print('发送超时');
            #Device_State=0#恢复到未连接状态
            #ser.close()#将串口关闭，防止下次无法打开
        #else:
            #print('发送完成');
    except:#出现异常
        #print('发送异常');
        Device_State=0
        ser.close()#将串口关闭，防止下次无法打开
        
def SER_Read():
    global Device_State
    #print('接收数据ing');
    try:#尝试获取数据
        Data_U1=ser.read(ser.in_waiting)
        return Data_U1
    except:#出现异常
        #print('接收异常');
        Device_State=0
        ser.close()#将串口关闭，防止下次无法打开
        return 0
    




def Read_M_u8(add):#读取主机u8寄存器（MSC设备编码，Add）
    hex_use=bytearray()#空数组
    hex_use.append(0)#发给主机
    hex_use.append(48)#识别为SFR指令
    hex_use.append(0*32)#识别为8bit SFR读
    hex_use.append(add//256)#高地址
    hex_use.append(add%256)#低地址
    hex_use.append(0)#数值
    SER_Write(hex_use)#发出指令
    
    #等待收回信息
    while(1):
        recv = SER_Read()#.decode("byte")#获取串口数据
        if(recv==0):
            return 0
        elif(len(recv)!=0):
            return recv[5]

def Read_M_u16(add):#读取主机u8寄存器（MSC设备编码，Add）
    hex_use=bytearray()#空数组
    hex_use.append(0)#发给主机
    hex_use.append(48)#识别为SFR指令
    hex_use.append(1*32)#识别为16bit SFR读
    hex_use.append(add%256)#地址
    hex_use.append(0)#高位数值
    hex_use.append(0)#低位数值
    SER_Write(hex_use)#发出指令
    #等待收回信息
    while(1):
        recv = SER_Read()#.decode("gbk")#获取串口数据
        if(recv==0):
            return 0
        elif(len(recv)!=0):
            return recv[4]*256+recv[5]

def Write_M_u8(add,data_w):#读取主机u8寄存器（MSC设备编码，Add）
    hex_use=bytearray()#空数组
    hex_use.append(0)#发给主机
    hex_use.append(48)#识别为SFR指令
    hex_use.append(4*32)#识别为16bit SFR写
    hex_use.append(add//256)#高地址
    hex_use.append(add%256)#低地址
    hex_use.append(data_w%256)#数值
    SER_Write(hex_use)#发出指令
    #等待收回信息
    while(1):
        recv = SER_Read()#.decode("UTF-8")#获取串口数据
        if(recv==0):
            return 0
        elif(len(recv)!=0):
            break
            #return recv[5]

def Write_M_u16(add,data_w):#读取主机u8寄存器（MSC设备编码，Add）
    hex_use=bytearray()#空数组
    hex_use.append(0)#发给主机
    hex_use.append(48)#识别为SFR指令
    hex_use.append(1*32)#识别为16bit SFR写
    hex_use.append(add%256)#地址
    hex_use.append(data_w//256)#高位数值
    hex_use.append(data_w%256)#低位数值
    SER_Write(hex_use)#发出指令
    #等待收回信息
    while(1):
        recv = SER_Read()#.decode("gbk")#获取串口数据
        if(recv==0):
            return 0
        elif(len(recv)!=0):
            break

def Read_ADC_CH(ch):#读取主机ADC寄存器数值（ADC通道）
    hex_use=bytearray()#空数组
    hex_use.append(8)#读取ADC
    hex_use.append(ch)#通道
    hex_use.append(0)
    hex_use.append(0)
    hex_use.append(0)
    hex_use.append(0)
    SER_Write(hex_use)#发出指令
    #等待收回信息
    while(1):
        recv = SER_Read()#.decode("gbk")#获取串口数据
        if(recv==0):
            return 0
        elif(len(recv)!=0):
            return recv[4]*256+recv[5]
            
def Read_M_SFR_Data(add):#从u8区域获取SFR描述
    SFR_data=bytearray()#空数组
    for i in range(0,256):#以128字节为单位进行解析编码
        SFR_data.append(Read_M_u8(add+i))#读取编码数据
    data_type=0#根据是否为0进行类型循环统计
    data_num=0
    data_len=0
    data_use=bytearray()#空数组
    data_name=b''
    data_unit=b''
    data_family=b''
    data_data=b''
    for i in range(0,256):#以128字节为单位进行解析编码
        if(SFR_data[i]!=0 and data_type<3):
            data_use.append(SFR_data[i])#将非0数据合并到一块
        elif(data_type<3):#检测到0且未超纲
            if(len(data_use)==0):#没有接收到数据时就接收到00
                break#检测到0后收集的数据为空，判断为结束
            if(data_type==0):
                data_name=data_use#名称
                data_type=1
            elif(data_type==1):
                data_unit=data_use#单位
                data_type=2
            elif(data_type==2):
                data_family=data_use#类型
                data_type=3
                if(int(ord(data_use)//32)==0):#u8 data 2B add
                    data_len=2
                elif(int(ord(data_use)//32)==1):#u16 data 1B add
                    data_len=1
                elif(int(ord(data_use)//32)==2):#u32 data 2B add
                    data_len=2
                elif(int(ord(data_use)//32)==3):#u8 Text XB data
                    data_len=data_family[0]%32#计算数据长度
            data_use=bytearray()#空数组
            continue #进行下一次循环
        if(data_len>0  and data_type==3):#正式的有效数据
            data_use.append(SFR_data[i])#将非0数据合并到一块
            data_len=data_len-1
        if(data_len==0 and data_type==3):#将后续数据收集完整
            data_data=data_use
            data_type=0#重置类型
            My_MSN_Data.append(MSN_Data(data_name,data_unit,data_family,data_data))#对数据进行登记
            data_use=bytearray()#空数组



def Print_MSN_Data():
    num=len(My_MSN_Data)
    data_str=''
    print('MSN数据总数为：'+str(num))
    #进行数据解析
    for i in range(0,num):#将数据全部打印出来
        data_str=data_str+'序号：'+str(i)+'    名称：'+str(My_MSN_Data[i].name)+'    单位:'+str(My_MSN_Data[i].unit)
        if(ord(My_MSN_Data[i].family)//32==0):#数据类型为u8地址(16bit)
            data_str=data_str+'    类型：u8_SFR地址,长度'+str(ord(My_MSN_Data[i].family)%32)
            data_str=data_str+'    地址：'+str(int(My_MSN_Data[i].data[0])*256+int(My_MSN_Data[i].data[1]))
        elif(ord(My_MSN_Data[i].family)//32==1):#数据类型为u16地址(8bit)
            data_str=data_str+'    类型：u16_SFR地址,长度'+str(ord(My_MSN_Data[i].family)%32)
            data_str=data_str+'    地址：'+str(int(My_MSN_Data[i].data[0]))
        elif(ord(My_MSN_Data[i].family)//32==2):#数据类型为u32地址(16bit)
            data_str=data_str+'    类型：u32_SFR地址,长度：'+str(ord(My_MSN_Data[i].family)%32)
            data_str=data_str+'    地址：'+ str(int(My_MSN_Data[i].data[0])*256+int(My_MSN_Data[i].data[1]))
        elif(ord(My_MSN_Data[i].family)//32==3):#数据类型为u8字符串
            data_str=data_str+'    类型：字符串,长度'+str(ord(My_MSN_Data[i].family)%32)
            data_str=data_str+'    数据：'+str(My_MSN_Data[i].data)
        elif(ord(My_MSN_Data[i].family)//32==4):#数据类型为u8数组
            data_str=data_str+'    类型：u8数组数据,长度'+str(int(My_MSN_Data[i].family)%32)
            data_str=data_str+'    数据：'+str(My_MSN_Data[i].data)
        print(data_str)
        data_str=''

def Read_MSN_Data(name_use):#读取MSN_data中的数据
    num=len(My_MSN_Data)
    use_data=[]#创建一个空列表
    for i in range(0,num):#将数据查找一遍
        if(My_MSN_Data[i].name == name_use):
            if(ord(My_MSN_Data[i].family)//32==0):#数据类型为u8地址(16bit)
                sfr_add=int(My_MSN_Data[i].data[0])*256+int(My_MSN_Data[i].data[1])
                for n in range(0,ord(My_MSN_Data[i].family)%32):
                    use_data.append(Read_M_u8(sfr_add+n))
            elif(ord(My_MSN_Data[i].family)//32==1): #数据类型为u16地址(8bit)
                use_data=Read_M_u16(int(My_MSN_Data[i].data[0]))
            elif(ord(My_MSN_Data[i].family)//32==3): #数据类型为u8字符串
                use_data=My_MSN_Data[i].data
            elif(ord(My_MSN_Data[i].family)//32==4):#数据类型为u8数组
                use_data=My_MSN_Data[i].data
            print(str(My_MSN_Data[i].name)+'='+str(use_data))
            return use_data
    if name_use!=0:
        print('请检查名称是否正确')
    return 0

def Write_MSN_Data(name_use,data_w):#在MSN_data写入数据
    num=len(My_MSN_Data)
    for i in range(0,num):#将数据查找一遍
        if(My_MSN_Data[i].name == name_use):
            if(int(My_MSN_Data[i].family)//32==0):#数据类型为u8地址(16bit)
                Write_M_u8(int(My_MSN_Data[i].data[0])*256+int(My_MSN_Data[i].data[1]),data_w)
                print('"'+name_use+'"'+'写入'+str(data_w)+'完成')
                return 0
            elif(int(My_MSN_Data[i].family)//32==1): #数据类型为u16地址(8bit)
                Write_M_u16(int(My_MSN_Data[i].data[0]),data_w)
                print('"'+name_use+'"'+'写入'+str(data_w)+'完成')
                return 0
    print('"'+name_use+'"'+'不存在,请检查名称是否正确')

def Write_Flash_Page(Page_add,data_w,Page_num):#往Flash指定页写入256B数据
    #先把数据传输完成
    hex_use=bytearray()#空数组
    for i in range(0,64):#256字节数据分为64个指令
        hex_use.append(4)#多次写入Flash
        hex_use.append(i)#低位地址
        hex_use.append(data_w[i*4+0])#Data0
        hex_use.append(data_w[i*4+1])#Data1
        hex_use.append(data_w[i*4+2])#Data2
        hex_use.append(data_w[i*4+3])#Data3
        SER_Write(hex_use)#发出指令
    hex_use=bytearray()#空数组
    hex_use.append(3)#对Flash操作
    hex_use.append(1)#写Flash
    hex_use.append(Page_add//(65536))#Data0
    hex_use.append((Page_add%65536)//256)#Data1
    hex_use.append((Page_add%65536)%256)#Data2
    hex_use.append(Page_num%256)#Data3
    SER_Write(hex_use)#发出指令
    #等待收回信息
    while(1):
        recv = SER_Read()#.decode("UTF-8")#获取串口数据
        if(recv==0):
            return 0
        elif(len(recv)!=0):
            break

def Write_Flash_Page_fast(Page_add,data_w,Page_num):#未经过擦除，直接往Flash指定页写入256B数据
    #先把数据传输完成
    hex_use=b''
    for i in range(0,64):#256字节数据分为64个指令
        hex_use=hex_use+int(4).to_bytes(1,byteorder="little")#多次写入Flash
        hex_use=hex_use+int(i).to_bytes(1,byteorder="little")#低位地址
        hex_use=hex_use+data_w[i*4+0].to_bytes(1,byteorder="little")#Data0
        hex_use=hex_use+data_w[i*4+1].to_bytes(1,byteorder="little")#Data1
        hex_use=hex_use+data_w[i*4+2].to_bytes(1,byteorder="little")#Data2
        hex_use=hex_use+data_w[i*4+3].to_bytes(1,byteorder="little")#Data3
    hex_use=hex_use+int(3).to_bytes(1,byteorder="little")#对Flash操作
    hex_use=hex_use+int(3).to_bytes(1,byteorder="little")#经过擦除，写Flash
    hex_use=hex_use+int(Page_add//(256*256)).to_bytes(1,byteorder="little")#Data0
    hex_use=hex_use+int((Page_add%65536)//256).to_bytes(1,byteorder="little")#Data1
    hex_use=hex_use+int((Page_add%65536)%256).to_bytes(1,byteorder="little")#Data2
    hex_use=hex_use+int(Page_num).to_bytes(1,byteorder="little")#Data3
    SER_Write(hex_use)#发出指令
    #等待收回信息
    while(1):
        recv = SER_Read()#.decode("UTF-8")#获取串口数据
        if(recv==0):
            return 0
        elif(len(recv)!=0):
            break

def Write_Flash_Page_fast_Max(Page_add,data_w,Page_num):#未经过擦除，直接往Flash指定页写入256B数据
    buffer = bytearray(360)# 预分配足够空间的bytearray (64*6 + 5 = 389字节)
    pos = 0
    for i in range(64):# 构建64个数据写入指令
        buffer[pos] = 4  # 写入指令
        buffer[pos+1] = i  # 低位地址
        # 一次性复制4字节数据
        start_idx = i*4
        buffer[pos+2] = data_w[start_idx]
        buffer[pos+3] = data_w[start_idx+1]
        buffer[pos+4] = data_w[start_idx+2]
        buffer[pos+5] = data_w[start_idx+3]
        pos += 6
    # 构建Flash操作指令
    buffer[pos] = 3    # 操作指令
    buffer[pos+1] = 3  # 写Flash指令(带擦除)
    # 分解24位地址
    buffer[pos+2] = Page_add >> 16        # 地址高字节
    buffer[pos+3] = (Page_add >> 8) & 0xFF # 地址中字节
    buffer[pos+4] = Page_add & 0xFF       # 地址低字节
    buffer[pos+5] = Page_num              # 页面编号
    pos += 6
    SER_Write(bytes(buffer[:pos]))# 发送完整指令
    
    
    

def Erase_Flash_page(add,size):#清空指定区域的内存
    hex_use=bytearray()#空数组
    hex_use.append(3)#对Flash操作
    hex_use.append(2)#清空指定区域的内存
    hex_use.append((add%65536)//256)#Data1
    hex_use.append((add%65536)%256)#Data2
    hex_use.append((size%65536)//256)#Data1
    hex_use.append((size%65536)%256)#Data2
    SER_Write(hex_use)#发出指令
    #等待收回信息
    while(1):
        recv = SER_Read()#.decode("UTF-8")#获取串口数据
        if(recv==0):
            return 0
        elif(len(recv)!=0):
            break




def Read_Flash_byte(add):#读取指定地址的数值
    hex_use=bytearray()#空数组
    hex_use.append(3)#对Flash操作
    hex_use.append(0)#读Flash
    hex_use.append(add//(256*256))#Data0
    hex_use.append((add%65536)//256)#Data1
    hex_use.append((add%65536)%256)#Data2
    hex_use.append(0)#Data3
    SER_Write(hex_use)#发出指令
    #等待收回信息
    while(1):
        recv = SER_Read()#.decode("UTF-8")#获取串口数据
        if(recv==0):
            return 0
        elif(len(recv)!=0):
            print(recv[5])
            return recv[5]
            
def Write_Flash_FS_Add(add):#配置进行高速写入
    hex_use=bytearray()#空数组
    hex_use.append(3)#对Flash操作
    hex_use.append(4)#高速写入
    hex_use.append((add%65536)//256)#Data1
    hex_use.append((add%65536)%256)#Data2
    hex_use.append(0)#Data3
    hex_use.append(0)#Data4
    SER_Write(hex_use)#发出指令
    #等待收回信息
    while(1):
        recv = SER_Read()#.decode("UTF-8")#获取串口数据
        if(recv==0):
            return 0
        elif(len(recv)!=0):
            break
            
            
    
def Write_Flash_Photo_fast(Page_add,Photo_name):#往Flash里面写入Bin格式的照片
    global Text1
    filepath=Photo_name+'.bin'#合成文件名称
    try:#尝试打开bin文件
        binfile=open(filepath,'rb')#以只读方式打开
    except:#出现异常
        print('找不到“'+filepath+'”文件,请检查其位置是否位于当前目录下');
        #Text1.delete(1.0,END)#清除文本框
        Text1.insert(END,'文件路径或格式出错!\n')#在文本框开始位置插入“内容一”
        return 0
    Fsize=os.path.getsize(filepath)
    print('找到“'+filepath+'”文件,大小：'+str(Fsize)+' B');
    Text1.insert(END,'大小'+str(Fsize)+'B,烧录中...\n')#在文本框开始位置插入“内容一”
    u_time=time.time()
    #进行擦除(W25Q64确保必须是4K空间)
    if(Fsize%4096 !=0):
        Erase_Flash_page(Page_add,(Fsize//4096+1)*16)#清空指定区域的内存
    else:
        Erase_Flash_page(Page_add,(Fsize//4096)*16)#清空指定区域的内存
    
    for i in range(0,Fsize//256):#每次写入一个Page
        Fdata=binfile.read(256)
        Write_Flash_Page_fast(Page_add+i,Fdata,1)#(page,数据，大小)
    if(Fsize%256 !=0):#还存在没写完的数据
        Fdata=binfile.read(Fsize%256)#将剩下的数据读完
        for i in range(Fsize%256,256):
            Fdata=Fdata+int(255).to_bytes(1,byteorder="little")#不足位置补充0xFF
        Write_Flash_Page_fast(Page_add+Fsize//256,Fdata,1)#(page,数据，大小)
    u_time=time.time()-u_time
    
    print(filepath+' 烧写完成,耗时'+str(u_time)+'秒')
    Text1.insert(END,'烧写完成,耗时'+str(int(u_time*1000))+'ms\n')#在文本框开始位置插入“内容一”
    
    
    
    
def Write_Flash_hex_fast_Max(Page_add,img_use):#往Flash里面写入hex数据
    Fsize=len(img_use)
    Text1.insert(END,'大小'+str(Fsize)+'B,烧录中...\n')#在文本框开始位置插入“内容一”
    u_time=time.time()
    
    #进行擦除(W25Q64确保必须是4K空间)
    #if(Fsize%4096 !=0):
    #    Erase_Flash_page(Page_add,(Fsize//4096+1)*16)#清空指定区域的内存
    #else:
    #    Erase_Flash_page(Page_add,(Fsize//4096)*16)#清空指定区域的内存
    Write_Flash_FS_Add(Page_add)
    
    #hex_16RGB=RGB.reshape(-1)#转成u16形式
    
    buffer_data=create_string_buffer(len(img_use)*2)#创建足够大的缓存区
    
    #rgb_ptr = img_use.ctypes.data_as(POINTER(c_uint16))#指针转换(可能出Bug)
    
    buf = create_string_buffer(bytes(img_use), len(img_use))# 转换为uint16指针
    rgb_ptr = cast(buf, POINTER(c_uint16))
    buf_prt = cast(buffer_data, POINTER(c_ubyte))
    len_dat =  c_uint32(len(img_use)//2)
    
    num=lib.MSN_compaction(rgb_ptr,buf_prt,len_dat)#数据压缩函数
    uart_data=bytes(buffer_data[:num])
    u_time=time.time()-u_time
    Text1.insert(END,'压缩完成,耗时'+str(int(u_time*1000))+'ms\n')#在文本框开始位置插入“内容一”
    u_time=time.time()
    #print
    ser.write(uart_data)
    u_time=time.time()-u_time
    Text1.insert(END,'烧写完成,耗时'+str(int(u_time*1000))+'ms\n')#在文本框开始位置插入“内容一”
    
    
    
def Write_Flash_hex_fast(Page_add,img_use):#往Flash里面写入hex数据
    Fsize=len(img_use)
    Text1.insert(END,'大小'+str(Fsize)+'B,烧录中...\n')#在文本框开始位置插入“内容一”
    u_time=time.time()
    #进行擦除(W25Q64确保必须是4K空间)
    if(Fsize%4096 !=0):
        Erase_Flash_page(Page_add,(Fsize//4096+1)*16)#清空指定区域的内存
    else:
        Erase_Flash_page(Page_add,(Fsize//4096)*16)#清空指定区域的内存
    
    for i in range(0,Fsize//256):#每次写入一个Page
        Fdata=img_use[:256]#取前256字节
        img_use=img_use[256:]#取剩余字节
        Write_Flash_Page_fast(Page_add+i,Fdata,1)#(page,数据，大小)
    if(Fsize%256 !=0):#还存在没写完的数据
        Fdata=img_use#将剩下的数据读完
        for i in range(Fsize%256,256):
            Fdata=Fdata+int(255).to_bytes(1,byteorder="little")#不足位置补充0xFF
        Write_Flash_Page_fast(Page_add+Fsize//256,Fdata,1)#(page,数据，大小)
    u_time=time.time()-u_time
    Text1.insert(END,'烧写完成,耗时'+str(int(u_time*1000))+'ms\n')#在文本框开始位置插入“内容一”
    
    
    
def Write_Flash_ZK(Page_add,ZK_name):#往Flash里面写入Bin格式的字库
    filepath=ZK_name+'.bin'#合成文件名称
    try:#尝试打开bin文件
        binfile=open(filepath,'rb')#以只读方式打开
    except:#出现异常
        print('找不到“'+filepath+'”文件,请检查其位置是否位于当前目录下');
        return 0
    Fsize=os.path.getsize(filepath)-6#字库文件的最后六个字节不是点阵信息
    print('找到“'+filepath+'”文件,大小：'+str(Fsize)+' B');
    for i in range(0,Fsize//256):#每次写入一个Page
        Fdata=binfile.read(256)
        Write_Flash_Page(Page_add+i,Fdata,1)#(page,数据，大小)
    if(Fsize%256 !=0):#还存在没写完的数据
        Fdata=binfile.read(Fsize%256)#将剩下的数据读完
        for i in range(Fsize%256,256):
            Fdata=Fdata+int(255).to_bytes(1,byteorder="little")#不足位置补充0xFF
        Write_Flash_Page(Page_add+Fsize//256,Fdata,1)#(page,数据，大小)
    print(filepath+' 烧写完成')

def LCD_Set_XY(LCD_D0,LCD_D1):#设置起始位置
    hex_use=int(2).to_bytes(1,byteorder="little")#对LCD多次写入
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")#设置起始位置
    hex_use=hex_use+int(LCD_D0//256).to_bytes(1,byteorder="little")#Data0
    hex_use=hex_use+int(LCD_D0%256).to_bytes(1,byteorder="little")#Data1
    hex_use=hex_use+int(LCD_D1//256).to_bytes(1,byteorder="little")#Data2
    hex_use=hex_use+int(LCD_D1%256).to_bytes(1,byteorder="little")#Data3
    SER_Write(hex_use)#发出指令

def RGB_LED_Set_Colour(LED_num,R_num,G_num,B_num):#
    hex_use=int(2).to_bytes(1,byteorder="little")#对LCD多次写入
    hex_use=hex_use+int(5).to_bytes(1,byteorder="little")#RGB灯珠的序号
    hex_use=hex_use+int(LED_num%64).to_bytes(1,byteorder="little")#RGB灯珠的序号,目前仅支持64个灯珠
    hex_use=hex_use+int(R_num%256).to_bytes(1,byteorder="little")#R
    hex_use=hex_use+int(G_num%256).to_bytes(1,byteorder="little")#G
    hex_use=hex_use+int(B_num%256).to_bytes(1,byteorder="little")#B
    SER_Write(hex_use)#发出指令
def RGB_LED_Up_Data():#更新RGB灯珠的值
    hex_use=int(2).to_bytes(1,byteorder="little")#对LCD多次写入
    hex_use=hex_use+int(3).to_bytes(1,byteorder="little")#解析命令
    hex_use=hex_use+int(12).to_bytes(1,byteorder="little")#更新RGB灯珠的值
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")#
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")#
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")#
    SER_Write(hex_use)#发出指令
    
def RGB_LED_Set_All_Colour(R_num,G_num,B_num):#将64个RGB灯全部设置为指定的颜色
    hex_use=b''
    for i in range(0,64):
        hex_use=hex_use+int(2).to_bytes(1,byteorder="little")#对LCD多次写入
        hex_use=hex_use+int(5).to_bytes(1,byteorder="little")#RGB灯珠的序号
        hex_use=hex_use+int(i).to_bytes(1,byteorder="little")#RGB灯珠的序号,目前仅支持64个灯珠
        hex_use=hex_use+int(R_num%256).to_bytes(1,byteorder="little")#R
        hex_use=hex_use+int(G_num%256).to_bytes(1,byteorder="little")#G
        hex_use=hex_use+int(B_num%256).to_bytes(1,byteorder="little")#B
    hex_use=hex_use+int(2).to_bytes(1,byteorder="little")#对LCD多次写入
    hex_use=hex_use+int(3).to_bytes(1,byteorder="little")#解析命令
    hex_use=hex_use+int(12).to_bytes(1,byteorder="little")#更新RGB灯珠的值
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")#
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")#
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")#
    SER_Write(hex_use)#发出指令
    
    
def LCD_Set_Size(LCD_D0,LCD_D1):#设置大小
    hex_use=int(2).to_bytes(1,byteorder="little")#对LCD多次写入
    hex_use=hex_use+int(1).to_bytes(1,byteorder="little")#设置大小
    hex_use=hex_use+int(LCD_D0//256).to_bytes(1,byteorder="little")#Data0
    hex_use=hex_use+int(LCD_D0%256).to_bytes(1,byteorder="little")#Data1
    hex_use=hex_use+int(LCD_D1//256).to_bytes(1,byteorder="little")#Data2
    hex_use=hex_use+int(LCD_D1%256).to_bytes(1,byteorder="little")#Data3
    SER_Write(hex_use)#发出指令
    
def LCD_Set_Color(LCD_D0,LCD_D1):#设置颜色（FC,BC）
    hex_use=int(2).to_bytes(1,byteorder="little")#对LCD多次写入
    hex_use=hex_use+int(2).to_bytes(1,byteorder="little")#设置颜色
    hex_use=hex_use+int(LCD_D0//256).to_bytes(1,byteorder="little")#Data0
    hex_use=hex_use+int(LCD_D0%256).to_bytes(1,byteorder="little")#Data1
    hex_use=hex_use+int(LCD_D1//256).to_bytes(1,byteorder="little")#Data2
    hex_use=hex_use+int(LCD_D1%256).to_bytes(1,byteorder="little")#Data3
    SER_Write(hex_use)#发出指令
    
def LCD_Photo(LCD_X,LCD_Y,LCD_X_Size,LCD_Y_Size,Page_Add):#
    global Device_State
    LCD_Set_XY(LCD_X,LCD_Y)
    LCD_Set_Size(LCD_X_Size,LCD_Y_Size)
    hex_use=int(2).to_bytes(1,byteorder="little")#对LCD多次写入
    hex_use=hex_use+int(3).to_bytes(1,byteorder="little")#设置指令
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")#显示彩色图片
    hex_use=hex_use+int(Page_Add//256).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(Page_Add%256).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")
    SER_Write(hex_use)#发出指令
    #等待收回信息
    while(1):
        time.sleep(0.001)
        recv = SER_Read()#.decode("UTF-8")#获取串口数据
        if(recv==0):
            return 0
        elif(len(recv)!=0):
            if((recv[0]!=hex_use[0]) or (recv[1]!=hex_use[1])):
                Device_State=0#接收出错
            break

def LCD_ADD(LCD_X,LCD_Y,LCD_X_Size,LCD_Y_Size):#
    global Device_State
    LCD_Set_XY(LCD_X,LCD_Y)
    LCD_Set_Size(LCD_X_Size,LCD_Y_Size)
    hex_use=int(2).to_bytes(1,byteorder="little")#对LCD多次写入
    hex_use=hex_use+int(3).to_bytes(1,byteorder="little")#设置指令
    hex_use=hex_use+int(7).to_bytes(1,byteorder="little")#载入地址
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")
    SER_Write(hex_use)#发出指令
    #等待收回信息
    while(1):
        time.sleep(0.001)
        recv = SER_Read()#.decode("UTF-8")#获取串口数据
        if(recv==0):
            return 0
        elif(len(recv)!=0):
            if((recv[0]!=hex_use[0]) or (recv[1]!=hex_use[1])):
                Device_State=0#接收出错
            break

def LCD_State(LCD_S):#
    global Device_State
    hex_use=int(2).to_bytes(1,byteorder="little")#对LCD多次写入
    hex_use=hex_use+int(3).to_bytes(1,byteorder="little")#设置指令
    hex_use=hex_use+int(10).to_bytes(1,byteorder="little")#载入地址
    hex_use=hex_use+int(LCD_S).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")
    SER_Write(hex_use)#发出指令
    #等待收回信息
    while(1):
        time.sleep(0.001)
        recv = SER_Read()#.decode("UTF-8")#获取串口数据
        if(recv==0):
            return 0
        elif(len(recv)!=0):
            if((recv[0]!=hex_use[0]) or (recv[1]!=hex_use[1])):
                Device_State=0#接收出错
            break
            
            
            
def LCD_DATA(data_w,size):#往LCD写入指定大小的数据
    #先把数据传输完成
    hex_use=b''
    for i in range(0,64):#256字节数据分为64个指令
        hex_use=hex_use+int(4).to_bytes(1,byteorder="little")#多次写入Flash
        hex_use=hex_use+int(i).to_bytes(1,byteorder="little")#低位地址
        hex_use=hex_use+data_w[i*4+0].to_bytes(1,byteorder="little")#Data0
        hex_use=hex_use+data_w[i*4+1].to_bytes(1,byteorder="little")#Data1
        hex_use=hex_use+data_w[i*4+2].to_bytes(1,byteorder="little")#Data2
        hex_use=hex_use+data_w[i*4+3].to_bytes(1,byteorder="little")#Data3
    hex_use=hex_use+int(2).to_bytes(1,byteorder="little")#对Flash操作
    hex_use=hex_use+int(3).to_bytes(1,byteorder="little")#经过擦除，写Flash
    hex_use=hex_use+int(8).to_bytes(1,byteorder="little")#Data0
    hex_use=hex_use+int(size//256).to_bytes(1,byteorder="little")#Data1
    hex_use=hex_use+int(size%256).to_bytes(1,byteorder="little")#Data2
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")#Data3
    SER_Write(hex_use)#发出指令
   
def Write_LCD_Photo_fast(x_star,y_star,x_size,y_size,Photo_name):#往Flash里面写入Bin格式的照片
    filepath=Photo_name+'.bin'#合成文件名称
    try:#尝试打开bin文件
        binfile=open(filepath,'rb')#以只读方式打开
    except:#出现异常
        print('找不到“'+filepath+'”文件,请检查其位置是否位于当前目录下');
        return 0
    Fsize=os.path.getsize(filepath)
    print('找到“'+filepath+'”文件,大小：'+str(Fsize)+' B');
    u_time=time.time()
    #进行地址写入
    LCD_ADD(x_star,y_star,x_size,y_size)
    for i in range(0,Fsize//256):#每次写入一个Page
        Fdata=binfile.read(256)
        LCD_DATA(Fdata,256)#(page,数据，大小)
    if(Fsize%256 !=0):#还存在没写完的数据
        Fdata=binfile.read(Fsize%256)#将剩下的数据读完
        for i in range(Fsize%256,256):
            Fdata=Fdata+int(255).to_bytes(1,byteorder="little")#不足位置补充0xFF
        LCD_DATA(Fdata,Fsize%256)#(page,数据，大小)
    u_time=time.time()-u_time
    print(filepath+' 显示完成,耗时'+str(u_time)+'秒')

def Write_LCD_Photo_fast1(x_star,y_star,x_size,y_size,Photo_name):#往Flash里面写入Bin格式的照片
    filepath=Photo_name+'.bin'#合成文件名称
    try:#尝试打开bin文件
        binfile=open(filepath,'rb')#以只读方式打开
    except:#出现异常
        print('找不到“'+filepath+'”文件,请检查其位置是否位于当前目录下');
        return 0
    Fsize=os.path.getsize(filepath)
    print('找到“'+filepath+'”文件,大小：'+str(Fsize)+' B');
    u_time=time.time()
    #进行地址写入
    LCD_ADD(x_star,y_star,x_size,y_size)
    hex_use=bytearray()#空数组
    for j in range(0,Fsize//256):#每次写入一个Page
        data_w=binfile.read(256)
        #先把数据格式转换好
        for i in range(0,64):#256字节数据分为64个指令
            hex_use.append(4)
            hex_use.append(i)
            hex_use.append(data_w[i*4+0])
            hex_use.append(data_w[i*4+1])
            hex_use.append(data_w[i*4+2])
            hex_use.append(data_w[i*4+3])
        hex_use.append(2)
        hex_use.append(3)
        hex_use.append(8)
        hex_use.append(1)
        hex_use.append(0)
        hex_use.append(0)
    if(Fsize%256 !=0):#还存在没写完的数据
        data_w=binfile.read(Fsize%256)#将剩下的数据读完
        for i in range(Fsize%256,256):
            data_w=data_w+int(255).to_bytes(1,byteorder="little")#不足位置补充0xFF
        for i in range(0,64):#256字节数据分为64个指令
            hex_use.append(4)
            hex_use.append(i)
            hex_use.append(data_w[i*4+0])
            hex_use.append(data_w[i*4+1])
            hex_use.append(data_w[i*4+2])
            hex_use.append(data_w[i*4+3])
        hex_use.append(2)
        hex_use.append(3)
        hex_use.append(8)
        hex_use.append(0)
        hex_use.append(Fsize%256)
        hex_use.append(0)  
    hex_use.append(2)
    hex_use.append(3)
    hex_use.append(9)
    hex_use.append(0)
    hex_use.append(0)
    hex_use.append(0)
    SER_Write(hex_use)#发出指令
    u_time=time.time()-u_time
    print(filepath+' 显示完成,耗时'+str(u_time)+'秒')
    

def Write_LCD_Screen_fast(x_star,y_star,x_size,y_size,Photo_data):#往Flash里面写入Bin格式的照片
    LCD_ADD(x_star,y_star,x_size,y_size)
    Photo_data_use=Photo_data
    hex_use=bytearray()#空数组
    for j in range(0,x_size*y_size*2//256):#每次写入一个Page
        data_w=Photo_data_use[:256]
        Photo_data_use=Photo_data_use[256:]
        cmp_use=[]#空数组,
        for i in range(0,64):#256字节数据分为64个指令
            cmp_use.append(data_w[i*4+0]*256*256*256+data_w[i*4+1]*256*256+data_w[i*4+2]*256+data_w[i*4+3])
        result=max(set(cmp_use),key=cmp_use.count)#统计出现最多的数据
        hex_use.append(2)
        hex_use.append(4)
        color_ram=result
        hex_use.append(color_ram//(256*256*256))
        color_ram=color_ram%(256*256*256)
        hex_use.append(color_ram//(256*256))
        color_ram=color_ram%(256*256)
        hex_use.append(color_ram//256)
        hex_use.append(color_ram%256)
        #先把数据格式转换好
        for i in range(0,64):#256字节数据分为64个指令
            if((data_w[i*4+0]*256*256*256+data_w[i*4+1]*256*256+data_w[i*4+2]*256+data_w[i*4+3])!=result):#
                hex_use.append(4)
                hex_use.append(i)
                hex_use.append(data_w[i*4+0])
                hex_use.append(data_w[i*4+1])
                hex_use.append(data_w[i*4+2])
                hex_use.append(data_w[i*4+3])
        hex_use.append(2)
        hex_use.append(3)
        hex_use.append(8)
        hex_use.append(1)
        hex_use.append(0)
        hex_use.append(0)
    if(x_size*y_size*2%256 !=0):#还存在没写完的数据
        data_w=Photo_data_use#将剩下的数据读完
        for i in range(x_size*y_size*2%256,256):
            data_w.append(0xff)#不足位置补充0xFF
        for i in range(0,64):#256字节数据分为64个指令
            hex_use.append(4)
            hex_use.append(i)
            hex_use.append(data_w[i*4+0])
            hex_use.append(data_w[i*4+1])
            hex_use.append(data_w[i*4+2])
            hex_use.append(data_w[i*4+3])
        hex_use.append(2)
        hex_use.append(3)
        hex_use.append(8)
        hex_use.append(0)
        hex_use.append(x_size*y_size*2%256)
        hex_use.append(0)
    SER_Write(hex_use)#发出指令
    
#对发送的数据进行编码分析,缩短数据指令
def Write_LCD_Screen_fast1(x_star,y_star,x_size,y_size,Photo_data):#往Flash里面写入Bin格式的照片
    LCD_ADD(x_star,y_star,x_size,y_size)
    Photo_data_use=Photo_data
    hex_use=bytearray()#空数组
    for j in range(0,x_size*y_size*2//256):#每次写入一个Page
        data_w=Photo_data_use[:256]
        Photo_data_use=Photo_data_use[256:]
        #先把数据格式转换好
        for i in range(0,64):#256字节数据分为64个指令
            hex_use.append(4)
            hex_use.append(i)
            hex_use.append(data_w[i*4+0])
            hex_use.append(data_w[i*4+1])
            hex_use.append(data_w[i*4+2])
            hex_use.append(data_w[i*4+3])
        hex_use.append(2)
        hex_use.append(3)
        hex_use.append(8)
        hex_use.append(1)
        hex_use.append(0)
        hex_use.append(0)
    if(x_size*y_size*2%256 !=0):#还存在没写完的数据
        data_w=Photo_data_use#将剩下的数据读完
        for i in range(x_size*y_size*2%256,256):
            data_w.append(0xff)#不足位置补充0xFF
        for i in range(0,64):#256字节数据分为64个指令
            hex_use.append(4)
            hex_use.append(i)
            hex_use.append(data_w[i*4+0])
            hex_use.append(data_w[i*4+1])
            hex_use.append(data_w[i*4+2])
            hex_use.append(data_w[i*4+3])
        hex_use.append(2)
        hex_use.append(3)
        hex_use.append(8)
        hex_use.append(0)
        hex_use.append(x_size*y_size*2%256)
        hex_use.append(0)
    #等待传输完成
    hex_use.append(2)
    hex_use.append(3)
    hex_use.append(9)
    hex_use.append(0)
    hex_use.append(0)
    hex_use.append(0)
    SER_Write(hex_use)#发出指令
    
def LCD_Photo_wb(LCD_X,LCD_Y,LCD_X_Size,LCD_Y_Size,Page_Add,LCD_FC,LCD_BC):#
    global Device_State
    LCD_Set_XY(LCD_X,LCD_Y)
    LCD_Set_Size(LCD_X_Size,LCD_Y_Size)
    LCD_Set_Color(LCD_FC,LCD_BC)
    hex_use=int(2).to_bytes(1,byteorder="little")#对LCD多次写入
    hex_use=hex_use+int(3).to_bytes(1,byteorder="little")#设置指令
    hex_use=hex_use+int(1).to_bytes(1,byteorder="little")#显示单色图片
    hex_use=hex_use+int(Page_Add//256).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(Page_Add%256).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")
    SER_Write(hex_use)#发出指令
    #等待收回信息
    while(1):
        time.sleep(0.001)
        recv = SER_Read()#.decode("UTF-8")#获取串口数据
        if(recv==0):
            return 0
        elif(len(recv)!=0):#对于回传的数据需要进行校验，确保设备状态能够被准确识别到
            if((recv[0]!=hex_use[0]) or (recv[1]!=hex_use[1])):
                Device_State=0#接收出错
            break

def LCD_ASCII_32X64(LCD_X,LCD_Y,Txt,LCD_FC,LCD_BC,Num_Page):#
    global Device_State
    LCD_Set_XY(LCD_X,LCD_Y)
    LCD_Set_Color(LCD_FC,LCD_BC)
    hex_use=int(2).to_bytes(1,byteorder="little")#对LCD多次写入
    hex_use=hex_use+int(3).to_bytes(1,byteorder="little")#设置指令
    hex_use=hex_use+int(2).to_bytes(1,byteorder="little")#显示ASCII
    hex_use=hex_use+int(ord(Txt)).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(Num_Page//256).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(Num_Page%256).to_bytes(1,byteorder="little")
    SER_Write(hex_use)#发出指令
    #等待收回信息
    while(1):
        time.sleep(0.001)
        recv = SER_Read()#.decode("UTF-8")#获取串口数据
        if(recv==0):
            return 0
        elif(len(recv)!=0):
            if((recv[0]!=hex_use[0]) or (recv[1]!=hex_use[1])):
                Device_State=0#接收出错
            break
            
            

def LCD_GB2312_16X16(LCD_X,LCD_Y,Txt,LCD_FC,LCD_BC):#
    global Device_State
    LCD_Set_XY(LCD_X,LCD_Y)
    LCD_Set_Color(LCD_FC,LCD_BC)
    Txt_Data=Txt.encode('gb2312')
    hex_use=int(2).to_bytes(1,byteorder="little")#对LCD多次写入
    hex_use=hex_use+int(3).to_bytes(1,byteorder="little")#设置指令
    hex_use=hex_use+int(3).to_bytes(1,byteorder="little")#显示彩色图片
    hex_use=hex_use+int(Txt_Data[0]).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(Txt_Data[1]).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")
    SER_Write(hex_use)#发出指令
    #等待收回信息
    while(1):
        time.sleep(0.001)
        recv = SER_Read()#.decode("UTF-8")#获取串口数据
        if(recv==0):
            return 0
        elif(len(recv)!=0):
            if((recv[0]!=hex_use[0]) or (recv[1]!=hex_use[1])):
                Device_State=0#接收出错
            break

def LCD_Photo_wb_MIX(LCD_X,LCD_Y,LCD_X_Size,LCD_Y_Size,Page_Add,LCD_FC,BG_Page):#
    global Device_State
    LCD_Set_XY(LCD_X,LCD_Y)
    LCD_Set_Size(LCD_X_Size,LCD_Y_Size)
    LCD_Set_Color(LCD_FC,BG_Page)
    hex_use=int(2).to_bytes(1,byteorder="little")#对LCD多次写入
    hex_use=hex_use+int(3).to_bytes(1,byteorder="little")#设置指令
    hex_use=hex_use+int(4).to_bytes(1,byteorder="little")#显示单色图片
    hex_use=hex_use+int(Page_Add//256).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(Page_Add%256).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")
    SER_Write(hex_use)#发出指令
    #等待收回信息
    while(1):
        time.sleep(0.001)
        recv = SER_Read()#.decode("UTF-8")#获取串口数据
        if(recv==0):
            return 0
        elif(len(recv)!=0):
            if((recv[0]!=hex_use[0]) or (recv[1]!=hex_use[1])):
                Device_State=0#接收出错
            break

def LCD_ASCII_32X64_MIX(LCD_X,LCD_Y,Txt,LCD_FC,BG_Page,Num_Page):#
    global Device_State
    LCD_Set_XY(LCD_X,LCD_Y)
    LCD_Set_Color(LCD_FC,BG_Page)
    hex_use=int(2).to_bytes(1,byteorder="little")#对LCD多次写入
    hex_use=hex_use+int(3).to_bytes(1,byteorder="little")#设置指令
    hex_use=hex_use+int(5).to_bytes(1,byteorder="little")#显示ASCII
    hex_use=hex_use+int(ord(Txt)).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(Num_Page//256).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(Num_Page%256).to_bytes(1,byteorder="little")
    SER_Write(hex_use)#发出指令
    #等待收回信息
    while(1):
        #time.sleep(0.5)
        recv = SER_Read()#.decode("UTF-8")#获取串口数据
        if(recv==0):
            return 0
        elif(len(recv)!=0):
            if((recv[0]!=hex_use[0]) or (recv[1]!=hex_use[1])):
                Device_State=0#接收出错
            break

def LCD_GB2312_16X16_MIX(LCD_X,LCD_Y,Txt,LCD_FC,BG_Page):#
    global Device_State
    LCD_Set_XY(LCD_X,LCD_Y)
    LCD_Set_Color(LCD_FC,BG_Page)
    Txt_Data=Txt.encode('gb2312')
    #print(Txt_Data)
    hex_use=int(2).to_bytes(1,byteorder="little")#对LCD多次写入
    hex_use=hex_use+int(3).to_bytes(1,byteorder="little")#设置指令
    hex_use=hex_use+int(6).to_bytes(1,byteorder="little")#显示彩色图片
    hex_use=hex_use+int(Txt_Data[0]).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(Txt_Data[1]).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")
    SER_Write(hex_use)#发出指令
    #等待收回信息
    while(1):
        time.sleep(0.002)
        recv = SER_Read()#.decode("UTF-8")#获取串口数据
        if(recv==0):
            return 0
        elif(len(recv)!=0):
            if((recv[0]!=hex_use[0]) or (recv[1]!=hex_use[1])):
                Device_State=0#接收出错
            break



def LCD_Color_set(LCD_X,LCD_Y,LCD_X_Size,LCD_Y_Size,F_Color):#对指定区域进行颜色填充
    global Device_State
    LCD_Set_XY(LCD_X,LCD_Y)
    LCD_Set_Size(LCD_X_Size,LCD_Y_Size)
    hex_use=int(2).to_bytes(1,byteorder="little")#对LCD多次写入
    hex_use=hex_use+int(3).to_bytes(1,byteorder="little")#设置指令
    hex_use=hex_use+int(11).to_bytes(1,byteorder="little")#显示彩色图片
    hex_use=hex_use+int(F_Color//256).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(F_Color%256).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")
    SER_Write(hex_use)#发出指令
    #等待收回信息
    while(1):
        time.sleep(0.001)
        recv = SER_Read()#.decode("UTF-8")#获取串口数据
        if(recv==0):
            return 0
        elif(len(recv)!=0):
            if((recv[0]!=hex_use[0]) or (recv[1]!=hex_use[1])):
                Device_State=0#接收出错
            break

def LCD_RAM_Init(Data_RAM):#将单色缓冲区图像进行初始化
    hex_use=int(2).to_bytes(1,byteorder="little")#对LCD多次写入
    hex_use=hex_use+int(3).to_bytes(1,byteorder="little")#设置指令
    hex_use=hex_use+int(13).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(Data_RAM%256).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")
    SER_Write(hex_use)#发出指令







def LCD_Load_RAM_Mix_Show(LCD_FC,BG_Page):#将单色缓冲区图像和彩色背景图像混合输出
    global Device_State
    LCD_Set_Color(LCD_FC,0)
    hex_use=int(2).to_bytes(1,byteorder="little")#对LCD多次写入
    hex_use=hex_use+int(3).to_bytes(1,byteorder="little")#设置指令
    hex_use=hex_use+int(17).to_bytes(1,byteorder="little")
    color_ram=BG_Page%(256*256*256)
    hex_use=hex_use+int(color_ram//(256*256)).to_bytes(1,byteorder="little")
    color_ram=color_ram%(256*256)
    hex_use=hex_use+int(color_ram//256).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(color_ram%256).to_bytes(1,byteorder="little")
    SER_Write(hex_use)#发出指令

    


def LCD_Load_RAM_Show(LCD_FC,LCD_BC):#将单色缓冲区图像直接输出显示
    global Device_State
    LCD_Set_Color(LCD_FC,LCD_BC)
    hex_use=int(2).to_bytes(1,byteorder="little")#对LCD多次写入
    hex_use=hex_use+int(3).to_bytes(1,byteorder="little")#设置指令
    hex_use=hex_use+int(16).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(0).to_bytes(1,byteorder="little")
    SER_Write(hex_use)#发出指令

def LCD_Add_RAM(LCD_X,LCD_Y,LCD_X_Size,LCD_Y_Size,BG_Page):#将单色图像传输到缓冲区
    global Device_State
    LCD_Set_XY(LCD_X,LCD_Y)
    LCD_Set_Size(LCD_X_Size,LCD_Y_Size)
    hex_use=int(2).to_bytes(1,byteorder="little")#对LCD多次写入
    hex_use=hex_use+int(3).to_bytes(1,byteorder="little")#设置指令
    hex_use=hex_use+int(15).to_bytes(1,byteorder="little")
    color_ram=BG_Page%(256*256*256)
    hex_use=hex_use+int(color_ram//(256*256)).to_bytes(1,byteorder="little")
    color_ram=color_ram%(256*256)
    hex_use=hex_use+int(color_ram//256).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(color_ram%256).to_bytes(1,byteorder="little")
    SER_Write(hex_use)#发出指令
    

def LCD_Add_RAM_No_Mask(LCD_X,LCD_Y,LCD_X_Size,LCD_Y_Size,BG_Page):#将单色图像传输到缓冲区（不带掩膜）
    global Device_State
    LCD_Set_XY(LCD_X,LCD_Y)
    LCD_Set_Size(LCD_X_Size,LCD_Y_Size)
    hex_use=int(2).to_bytes(1,byteorder="little")#对LCD多次写入
    hex_use=hex_use+int(3).to_bytes(1,byteorder="little")#设置指令
    hex_use=hex_use+int(14).to_bytes(1,byteorder="little")
    color_ram=BG_Page%(256*256*256)
    hex_use=hex_use+int(color_ram//(256*256)).to_bytes(1,byteorder="little")
    color_ram=color_ram%(256*256)
    hex_use=hex_use+int(color_ram//256).to_bytes(1,byteorder="little")
    hex_use=hex_use+int(color_ram%256).to_bytes(1,byteorder="little")
    SER_Write(hex_use)#发出指令    
    
def HZ_to_Add(Txt):#将单个汉字GB编码转换为具体Flash地址
    Txt_Data=Txt.encode('gb2312')
    Add=((Txt_Data[0]-0xA0-1)*94+(Txt_Data[1]-0xA0-1))*32
    return Add

def show_gif():#显示GIF动图
    global State_change,gif_num,gif_data_num,gif_blk_num
    if(State_change==1):
        State_change=0
        gif_num=0
        gif_data_num=0
        gif_blk_num=0
        LCD_State(LCD_Change_now)#配置显示方向
        time.sleep(0.01)
    if(State_change==0):
        LCD_Photo(0,0,320,172,4096+430*gif_num)
        #LCD_Photo_wb_MIX(20,20,LCD_X_Size,LCD_Y_Size,Page_Add,LCD_FC,BG_Page)
        #LCD_GB2312_16X16_MIX(LCD_X,LCD_Y,Txt,LCD_FC,BG_Page)
        #LCD_GB2312_16X16(20,20,'墨',BLUE,RED)
        #LCD_GB2312_16X16_MIX(120,20,'墨',BLUE,4096+430*gif_num)
        #LCD_ASCII_32X64_MIX(120,10,chr(6+48),RED,4096+430*gif_num,3651)
        #LCD_Add_RAM(120,10,32,64,3651*256+(1+48)*256)#
        
        #LCD_Add_RAM(0,0,320,172,1024*256+gif_blk_num*430*16)
        
        #LCD_Add_RAM(160,80,16,16,HZ_to_Add('欣'))
        
        #LCD_Add_RAM_No_Mask(0,0,320,172,0x7B8000+0X36000)
        #LCD_Add_RAM_No_Mask(120,10,32,64,3651*256+(gif_data_num%10+48)*256)#
        
        #LCD_Add_RAM(88,37,72,100,3552*256+(gif_data_num//10)*1024)
        
        #LCD_Add_RAM_No_Mask(160,37,72,100,3552*256+(gif_data_num%10)*1024)
        
        #LCD_Load_RAM_Mix_Show(64832,4096*256+430*gif_num*256)
        #LCD_Load_RAM_Mix_Show(64832,0x7B8000+320*172*2)
        
        #LCD_Load_RAM_Show(WHITE,BLACK)
        #while(1):
        #    LCD_Photo(160,80,160,80,3926)#放置背景
        
        #LCD_Photo(160,80,160,80,3926)#放置背景
        gif_num=gif_num+1
        if(gif_num>63):
            gif_num=0
        
        gif_blk_num=gif_blk_num+1
        if(gif_blk_num>93):
            gif_blk_num=0
        
        gif_data_num=gif_data_num+1
        if(gif_data_num>99):
            gif_data_num=0
        time.sleep(0.012)
    #LCD_Color_set(40,0,80,80,RED)

def show_PC_state(FC,BC):#显示PC状态
    global State_change,page_now1,CPU
    photo_add=4038
    num_add=4026
    if(State_change==1):
        State_change=0
        page_now1=0
        LCD_State(LCD_Change_now)#配置显示方向
        LCD_RAM_Init(0x00)#清空缓冲区
        psutil.cpu_percent(interval=None)#非阻塞测量CPU占用率
    if(State_change==0):
        Add_X=(LCD_X-160)//2
        Add_Y=(LCD_Y-80)//2
        LCD_Add_RAM(0,0,320,172,1024*256+page_now1*430*16)#放置动图
        LCD_Add_RAM_No_Mask(0+Add_X,0+Add_Y,160,80,photo_add*256)#放置背景
        if(page_now1%15==0):
            CPU=int(psutil.cpu_percent(interval=None))
        
        mem=psutil.virtual_memory()
        RAM=int(mem.percent)
        
        battery = psutil.sensors_battery()
        if battery!=None :
            BAT=int(battery.percent)
        else:
            BAT=100
        FRQ=int(psutil.disk_usage('/').used*100/psutil.disk_usage('/').total)
        if(CPU>=100):
            LCD_Add_RAM_No_Mask(24+Add_X,0+Add_Y,8,33,(10+num_add)*256)
            CPU=CPU%100
        else:
            LCD_Add_RAM_No_Mask(24+Add_X,0+Add_Y,8,33,(11+num_add)*256)
        LCD_Add_RAM(32+Add_X,0+Add_Y,24,33,((CPU//10)+num_add)*256)
        LCD_Add_RAM(56+Add_X,0+Add_Y,24,33,((CPU%10)+num_add)*256)
        if(RAM>=100):
            LCD_Add_RAM_No_Mask(104+Add_X,0+Add_Y,8,33,(10+num_add)*256)
            RAM=RAM%100
        else:
            LCD_Add_RAM_No_Mask(104+Add_X,0+Add_Y,8,33,(11+num_add)*256)
        LCD_Add_RAM_No_Mask(112+Add_X,0+Add_Y,24,33,((RAM//10)+num_add)*256)
        LCD_Add_RAM_No_Mask(136+Add_X,0+Add_Y,24,33,((RAM%10)+num_add)*256)
        if(BAT>=100):
            LCD_Add_RAM_No_Mask(104+Add_X,47+Add_Y,8,33,(10+num_add)*256)
            BAT=BAT%100
        else:
            LCD_Add_RAM_No_Mask(104+Add_X,47+Add_Y,8,33,(11+num_add)*256)
        LCD_Add_RAM_No_Mask(112+Add_X,47+Add_Y,24,33,((BAT//10)+num_add)*256)
        LCD_Add_RAM_No_Mask(136+Add_X,47+Add_Y,24,33,((BAT%10)+num_add)*256)
        
        if(FRQ>=100):
            LCD_Add_RAM_No_Mask(24+Add_X,47+Add_Y,8,33,(10+num_add)*256)
            FRQ=FRQ%100
        else:
            LCD_Add_RAM_No_Mask(24+Add_X,47+Add_Y,8,33,(11+num_add)*256)
        LCD_Add_RAM_No_Mask(32+Add_X,47+Add_Y,24,33,((FRQ//10)+num_add)*256)
        LCD_Add_RAM_No_Mask(56+Add_X,47+Add_Y,24,33,((FRQ%10)+num_add)*256)
        
        LCD_Load_RAM_Show(FC,BC)
        page_now1=page_now1+1
        if(page_now1>93):#一共94张图片
            page_now1=0
        
    
def show_Photo1():#显示照片
    global State_change,Num_u
    FC=WHITE
    BC=BLACK
    num_add=3651
    photo_add=(0x7B8000+320*172*2)//256
    if(State_change==1):
        State_change=0
        Num_u=0
        LCD_State(LCD_Change_now)#配置显示方向
        LCD_Photo(0,0,320,172,photo_add)
        #LCD_ASCII_32X64_MIX(160-32*2,100,'T',FC,photo_add,num_add)
        #LCD_ASCII_32X64_MIX(160-32,100,'e',FC,photo_add,num_add)
        #LCD_ASCII_32X64_MIX(160,100,'m',FC,photo_add,num_add)
        #LCD_ASCII_32X64_MIX(160+32,100,'p',FC,photo_add,num_add)
        #LCD_Photo(0,0,160,80,3926)#放置背景
    if(State_change==0):
        #LCD_ASCII_32X64_MIX(160+32,100,'p',FC,photo_add,num_add)
        LCD_Photo(0,0,320,172,photo_add)
        #computer.Hardware[1].Update()
        #for sensor in computer.Hardware[1].Sensors:
        #    if(sensor.Name=="CPU Package"):
        #        if(int(sensor.SensorType)==2):#2对应温度
        #            #LCD_GB2312_16X16_MIX(16*10,8+64+16,str_num[int(sensor.Value)//10],FC,photo_add)
         #           #LCD_GB2312_16X16_MIX(16*11,8+64+16,str_num[int(sensor.Value)%10],FC,photo_add)
        #            LCD_Photo_wb_MIX(160-72,0,72,99,3552+(int(sensor.Value)//10)*4,FC,photo_add)
        #            LCD_Photo_wb_MIX(160,0,72,99,3552+(int(sensor.Value)%10)*4,FC,photo_add)
                    
                    
                    #LCD_Photo_wb_MIX(16*1,8+64+16,24,33,4026+int(sensor.Value)//10,FC,photo_add)
                    #LCD_Photo_wb_MIX(16*1+24,8+64+16,24,33,4026+int(sensor.Value)%10,FC,photo_add)
        
        #LCD_Photo_wb_MIX(0,0,48,66,3600+(Num_u%10)*2,FC,photo_add)
        
        Num_u=Num_u+1
        
        
        
        time.sleep(1)

def show_PC_time():
    global State_change
    FC=YELLOW
    photo_add=(0x7B8000)//256
    num_add=3651
    str_num="０１２３４５６７８９－"#全角字符
    if(State_change==1):
        State_change=0
        LCD_State(LCD_Change_now)#配置显示方向
        LCD_Photo(0,0,320,172,(0x7B8000)//256)
        psutil.cpu_percent(interval=None)#非阻塞测量CPU占用率
        #LCD_Photo(0,0,160,80,photo_add)#放置背景
        
        LCD_ASCII_32X64_MIX(16*9,8,':',FC,photo_add,num_add)
        #LCD_ASCII_32X64_MIX(136+8,32,':',FC,photo_add,num_add)
    if(State_change==0):
        time_h=int(datetime.now().hour)
        time_m=int(datetime.now().minute)
        time_S=int(datetime.now().second)
        time_Y=int(datetime.now().year)
        time_MM=int(datetime.now().month)
        time_D=int(datetime.now().day)
        #print(time_Y,time_MM,time_D)
        #print(datetime.now())
        LCD_ASCII_32X64_MIX(16*5,8,chr((time_h//10)+48),FC,photo_add,num_add)
        LCD_ASCII_32X64_MIX(16*7,8,chr((time_h%10)+48),FC,photo_add,num_add)
        LCD_ASCII_32X64_MIX(16*11,8,chr((time_m//10)+48),FC,photo_add,num_add)
        LCD_ASCII_32X64_MIX(16*13,8,chr((time_m%10)+48),FC,photo_add,num_add)
        
        LCD_GB2312_16X16_MIX(16*5,8+64,str_num[time_Y//1000],FC,photo_add)
        time_Y=time_Y%1000
        LCD_GB2312_16X16_MIX(16*6,8+64,str_num[time_Y//100],FC,photo_add)
        time_Y=time_Y%100
        LCD_GB2312_16X16_MIX(16*7,8+64,str_num[time_Y//10],FC,photo_add)
        LCD_GB2312_16X16_MIX(16*8,8+64,str_num[time_Y%10],FC,photo_add)
        LCD_GB2312_16X16_MIX(16*9,8+64,str_num[10],FC,photo_add)
        
        LCD_GB2312_16X16_MIX(16*10,8+64,str_num[time_MM//10],FC,photo_add)
        LCD_GB2312_16X16_MIX(16*11,8+64,str_num[time_MM%10],FC,photo_add)
        LCD_GB2312_16X16_MIX(16*12,8+64,str_num[10],FC,photo_add)
        LCD_GB2312_16X16_MIX(16*13,8+64,str_num[time_D//10],FC,photo_add)
        LCD_GB2312_16X16_MIX(16*14,8+64,str_num[time_D%10],FC,photo_add)
        
        #computer.Hardware[1].Update()
        #for sensor in computer.Hardware[1].Sensors:
            #if(sensor.Name=="CPU Package"):
                #if(int(sensor.SensorType)==2):#2对应温度
                    #LCD_GB2312_16X16_MIX(16*10,8+64+16,str_num[int(sensor.Value)//10],FC,photo_add)
                    #LCD_GB2312_16X16_MIX(16*11,8+64+16,str_num[int(sensor.Value)%10],FC,photo_add)
                    
                    ##time.sleep(0.001)
                    #LCD_GB2312_16X16_MIX(16*0,8+64+16,'温',FC,photo_add)
                    #LCD_GB2312_16X16_MIX(16*0,8+64+32,'度',FC,photo_add)
                    #LCD_Photo_wb_MIX(16*1,8+64+16,24,33,4026+int(sensor.Value)//10,FC,photo_add)
                    #LCD_Photo_wb_MIX(16*1+24,8+64+16,24,33,4026+int(sensor.Value)%10,FC,photo_add)
        
        
        
        
                    #LCD_Photo_wb(24,0,8,33,11+num_add,FC,BC)
                    #LCD_Photo_wb(24,0,8,33,11+num_add,FC,BC)
                    
                    #print("处理器温度："+str(sensor.Value))
        #LCD_ASCII_32X64_MIX(160+8,8,chr((time_S//10)+48),FC,photo_add,num_add)
        #LCD_ASCII_32X64_MIX(192+8,8,chr((time_S%10)+48),FC,photo_add,num_add)
        time.sleep(0.5)

def show_PC_Screen():#显示照片
    global State_change,Screen_Error,Device_State,Thread1
    global G_screnn0_OK,G_screnn1_OK,G_screnn0,G_screnn1,size_USE_X1,size_USE_Y1,size_USE_X0,size_USE_Y0
    if(State_change==1):
        State_change=0
        Screen_Error=0
        LCD_State(LCD_Change_now)#配置显示方向
        LCD_ADD(0,0,LCD_X,LCD_Y)#配置小屏幕显示范围
        #LCD_ADD(0,0,320,172)
        print('截图大小')
        while True:
            frame = camera.grab()  # 捕获当前帧
            if frame is not None:
                height, width, _ = frame.shape  # 形状为 (height, width, channels)
                print(f"图像尺寸: {width}x{height}")
                break
        if(width*LCD_Y>=height*LCD_X):#适配不同分辨率的屏幕，让其充满小屏幕显示区域
            size_USE_Y1=height
            size_USE_X1=height*LCD_X*2//LCD_Y//2#故意让X1,Y1均为偶数
            size_USE_Y0=0
            size_USE_X0=(width-size_USE_X1)//2
            size_USE_X1=size_USE_X1+size_USE_X0
        else:
            size_USE_X1=width
            size_USE_Y1=width*LCD_Y*2//LCD_X//2#故意让X1,Y1均为偶数
            size_USE_X0=0
            size_USE_Y0=(height-size_USE_Y1)//2
            size_USE_Y1=size_USE_Y1+size_USE_Y0
            print(size_USE_X0,size_USE_Y0,size_USE_X1,size_USE_Y1)
    #while(1):
    buffer_size = LCD_X*LCD_Y*10
    buffer_data = create_string_buffer(buffer_size)
    uart_data=bytearray()
    hex_16RGB=[]
    #while(1):
    delay_t=0
    u_time1=time.time()
    
    while True:  # 无条件进入循环
        try:#尝试建立屏幕截屏线程
            #print(size_USE_X0,size_USE_Y0,size_USE_X1,size_USE_Y1)
            im=camera.grab(region=(size_USE_X0,size_USE_Y0,size_USE_X1,size_USE_Y1))# 裁剪区域：y1:y2, x1:x2
        except:
            print("截取出错，重新配置显示区域")
            State_change=1#下次进入将重新初始化
            return None
        if im is not None:  # 当条件不满足时退出
            break
        #except:
            #time.sleep(0.001)#添加合理的sleep时间来降低功耗
            #print("屏幕分辨率发生变化，无法截取指定区域")
        
        time.sleep(0.001)#添加合理的sleep时间来降低功耗
        delay_t=delay_t+1;
        if(delay_t>50):
            #time.sleep(0.001)
            LCD_ADD(0,0,LCD_X,LCD_Y)#保持连接
            
    im2=cv2.resize(im, (LCD_X,LCD_Y), interpolation=cv2.INTER_AREA)#高质量图像缩小
    # 提取通道并转换为uint16避免溢出
    b = im2[:, :, 2].astype(np.uint16)  # BGR中的R通道
    g = im2[:, :, 1].astype(np.uint16)  # G通道
    r = im2[:, :, 0].astype(np.uint16)  # B通道
    # 位操作合并为RGB565（R:5位, G:6位, B:5位）
    RGB= ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3)
    hex_16RGB=RGB.reshape(-1)#转成u16形式
    rgb_ptr = hex_16RGB.ctypes.data_as(POINTER(c_uint16))#指针转换
    buf_prt = cast(buffer_data, POINTER(c_ubyte))
    len_dat =  c_uint32(len(hex_16RGB))
    num =  c_uint32(len(hex_16RGB))
    num=lib.MSN_compaction(rgb_ptr,buf_prt,len_dat)#数据压缩函数
    uart_data=bytes(buffer_data[:num])
    ser.write(uart_data)
    u_time1=time.time()-u_time1
    print("帧率："+str(1/u_time1))
    


def UI_Page():#进行图像界面显示
    global Label1,root,s1,s2,s3,Label2,Label3,Label4,Label5,Label6,Text1
    #创建主窗口
    root = tk.Tk() #实例化主窗口
    root.title("USB屏幕助手LITE版V1.2")#设置标题
    root.geometry(str(Show_W)+"x"+str(Show_H))#主窗口的大小以及在显示器上的位置
    #创建按键
    btn1=tk.Button(root,text="上翻页",height=1,width=8)
    btn1.place(x=400,y=275,anchor="w")#设置位置以及对齐方式
    btn1.config(command=Page_UP)#连接按键触发事件
    
    
    btn2=tk.Button(root,text="下翻页",height=1,width=8)
    btn2.place(x=400,y=325,anchor="w")#设置位置以及对齐方式
    btn2.config(command=Page_Down)#连接按键触发事件
    
    btn3=tk.Button(root,text="选择背景图像",height=1,width=12)
    btn3.place(x=250,y=75,anchor="w")#设置位置以及对齐方式
    btn3.config(command=Get_Photo_Path1)#连接按键触发事件
    
    btn4=tk.Button(root,text="选择闪存固件",height=1,width=12)
    btn4.place(x=250,y=125,anchor="w")#设置位置以及对齐方式
    btn4.config(command=Get_Photo_Path2)#连接按键触发事件
    
    btn5=tk.Button(root,text="烧写",height=1,width=8)
    btn5.place(x=400,y=75,anchor="w")#设置位置以及对齐方式
    btn5.config(command=Writet_Photo_Path1)#连接按键触发事件
    
    btn6=tk.Button(root,text="烧写",height=1,width=8)
    btn6.place(x=400,y=125,anchor="w")#设置位置以及对齐方式
    btn6.config(command=Writet_Photo_Path2)#连接按键触发事件
    
    btn7=tk.Button(root,text="切换显示方向",height=1,width=12)
    btn7.place(x=250,y=325,anchor="w")#设置位置以及对齐方式
    btn7.config(command=LCD_Change)#连接按键触发事件
    
    
    btn8=tk.Button(root,text="烧写",height=1,width=8)
    btn8.place(x=400,y=175,anchor="w")#设置位置以及对齐方式
    btn8.config(command=Writet_Photo_Path3)#连接按键触发事件
    
    btn9=tk.Button(root,text="烧写",height=1,width=8)
    btn9.place(x=400,y=225,anchor="w")#设置位置以及对齐方式
    btn9.config(command=Writet_Photo_Path4)#连接按键触发事件
    
    btn10=tk.Button(root,text="选择相册图像",height=1,width=12)
    btn10.place(x=250,y=175,anchor="w")#设置位置以及对齐方式
    btn10.config(command=Get_Photo_Path3)#连接按键触发事件
    
    btn11=tk.Button(root,text="选择动图文件",height=1,width=12)
    btn11.place(x=250,y=225,anchor="w")#设置位置以及对齐方式
    btn11.config(command=Get_Photo_Path4)#连接按键触发事件
    
    #创建滑块
    s1=Scale(root, from_=0, to=31,resolution=1,troughcolor='Red',orient=HORIZONTAL)#orient=HORIZONTAL 横向，默认纵向
    s1.place(x=150,y=25,anchor="w")#W.get()
    s1.set(31)
    s2=Scale(root, from_=0, to=63,resolution=1,troughcolor='Green',orient=HORIZONTAL)#orient=HORIZONTAL 横向，默认纵向
    s2.place(x=250,y=25,anchor="w")#W.get()可获取滑块值
    s2.set(0)
    s3=Scale(root, from_=0, to=31,resolution=1,troughcolor='Blue',orient=HORIZONTAL)#orient=HORIZONTAL 横向，默认纵向
    s3.place(x=350,y=25,anchor="w")#W.get()可获取滑块值
    s3.set(0)
    #创建标签
    Label1=tk.Label(root,text="设备未连接",bg="Red")
    Label1.place(x=25,y=25,anchor="w")#设置位置以及对齐方式
    
    Label2=tk.Label(root,bg="Red",width=2)
    Label2.place(x=460,y=25,anchor="w")#设置位置以及对齐方式
    
    Label3=tk.Label(root,bg="white",width=21)
    Label3.place(x=5,y=75,anchor="w")#设置位置以及对齐方式
    
    Label4=tk.Label(root,bg="white",width=21)
    Label4.place(x=5,y=125,anchor="w")#设置位置以及对齐方式
    
    
    
    Label5=tk.Label(root,bg="white",width=21)
    Label5.place(x=5,y=175,anchor="w")#设置位置以及对齐方式
    
    
    
    Label6=tk.Label(root,bg="white",width=21)
    Label6.place(x=5,y=225,anchor="w")#设置位置以及对齐方式
    
    
    
    #Text_Show=tk.StringVar()
    #Text_Show.set(0)
    #创建文本框
    Text1=tk.Text(root,width=23,height=4)
    Text1.place(x=5,y=300,anchor="w")#设置位置以及对齐方式
    #Text1.delete(0,END)
    #Text1.insert(END,'内容一')#在文本框开始位置插入“内容一”
    Text1.delete(1.0,END)#清除文本框

    #进入消息循环
    
    
    
    
    root.mainloop()
    



def Get_MSN_Device():#尝试获取MSN设备
    global Device_State,ADC_det,Thread1,ser,State_change,State_machine,My_MSN_Device,My_MSN_Data,Screen_Error,Label1,LCD_Change_now,Text1,LCD_X,LCD_Y
    port_list = list(serial.tools.list_ports.comports())#查询所有串口
    if len(port_list) == 0:
        print('未检测到串口,请确保设备已连接到电脑')
        #Label1.config(text="设备已连接",bg="GREEN")
        time.sleep(1)
        Label1.config(text="设备未连接",bg="RED")
        Device_State=0#未能连接
        try:#尝试建立屏幕截屏线程
            Thread1.stop()
        except:
            print("警告,无法关闭截图线程")
            
    else:#对串口进行监听，确保其为MSN设备
        My_MSN_Device=[]
        My_MSN_Data=[]
        for i in range(0,len(port_list)):
            try:#尝试打开串口
                ser=serial.Serial(port_list[i].name,921600,timeout =100, xonxoff=False,rtscts=True)      # 启用硬件流控（RTS/CTS）)#初始化串口连接,初始使用
            except:#出现异常
                print(port_list[i].name+'无法打开,请检查是否被其他程序占用');#显示MSN设备数量
                #ser.close()#将串口关闭，防止下次无法打开
                time.sleep(0.1)
                continue#执行下一次循环
            time.sleep(0.25)#理论上MSN设备100ms要发送一次“ MSN01”,在250ms内至少会收到一次
            recv = SER_Read()
            if(recv==0):
                break#退出当前for循环
            else:
                recv =recv.decode("gbk")#获取串口数据
            if(len(recv)>5):#收到6个字符以上数据时才进行解析
                for n in range(0,len(recv)-5):#逐字解析编码
                    if(ord(recv[n]) == 0):#当前字节为0时进行解析
                        if((recv[n+1] == 'M') and (recv[n+2] == 'S') and (recv[n+3] == 'N')):#确保为MSN设备
                            if((recv[n+4] >= '0') and (recv[n+4] <= '9') and (recv[n+5] >= '0')and (recv[n+5] <= '9')):#确保版本号为数字ASC码
                                My_MSN_Device.append(MSN_Device((port_list[i].name),(ord(recv[4])-48)*10+(ord(recv[5])-48)))#对MSN设备进行登记
                                hex_code=int(0).to_bytes(1,byteorder="little")#可以逐个加入数组
                                hex_code=hex_code+b'MSNCN'
                                SER_Write(hex_code)#返回消息
                                #等待返回消息，确认连接
                                time.sleep(0.25)#理论上MSN设备100ms要发送一次“ MSN01”,在250ms内至少会收到一次
                                recv = SER_Read().decode("gbk")#获取串口数据
                                if((ord(recv[0]) == 0) and (recv[1] == 'M') and (recv[2] == 'S') and (recv[3] == 'N') and (recv[4] == 'C') and (recv[5] == 'N')):#确保为MSN设备
                                    print('MSN设备'+str(len(My_MSN_Device))+'——'+port_list[i].name+'连接完成');#显示MSN设备数量
                                else:
                                    print('MSN设备'+str(len(My_MSN_Device))+'无法连接,请检查连接是否正常');#显示MSN设备数量
                                break#退出当前for循环
        print('MSN设备数量为'+str(len(My_MSN_Device))+'个');#显示MSN设备数量
        if(len(My_MSN_Device)>=1):
            Device_State=1#可以正常连接
            State_change=1#状态发生变化
            #State_machine=5#定义初始状态(保留上一次的状态)
            Screen_Error=0
            Read_M_SFR_Data(256)#读取u8在0x0100之后的128字节
            Print_MSN_Data()#解析字节中的数据格式
            Read_MSN_Data(b'MSN_Status')
            Read_MSN_Data(b'Flash_Info')
            
            UID=Read_MSN_Data(b'MSN_UID')
            LCD_X=Read_MSN_Data(b'Lcd_X')
            LCD_Y=Read_MSN_Data(b'Lcd_Y')
            print("SCRC")
            print(LCD_X,LCD_Y)
            ADC_det=Read_ADC_CH(9)
            ADC_det=(ADC_det+Read_ADC_CH(9))/2
            ADC_det=ADC_det-125#根据125的阈值判断是否被按下
            Label1.config(text="设备已连接",bg="GREEN")
            LCD_Change_now=0
            Text1.delete(1.0,END)#清除文本框
            #Text1.insert(END,'设备识别码:')#在文本框开始位置插入“内容一”
            
            #Label1=tk.Label(root,text="设备已连接",bg="GREEN")
            print("Flash")
            print(Read_Flash_byte(1024*4*256))
            
            #for i in range(1,37):
             #Write_Flash_Photo_fast(100*(i-1),str(i))#160*80分辨率彩色图片，占用100个Page

             #Write_Flash_Photo_fast(3600,'Demo1')#240*240单色图片，占用29个Page
             #Write_Flash_Photo_fast(3629,'N48X66P')#48*66分辨率数码管图像，占用22个Page
             #Write_Flash_ZK(3651,'ASC64_FBT')#32*64分辨率ASCII表格，占用128个Page
            
             #Write_Flash_Photo_fast(3779,'logo')#240*102单色LOGO,占用12个Page
             #Write_Flash_Photo_fast(3791,'J1')#240*240单色图片，占用29个Page
            
            #Write_Flash_Photo_fast(3820,'MLOGO')#160*68单色图片，占用6个Page
            #Write_Flash_Photo_fast(3826,'CLK_BG')#160*80彩色图片，占用100个Page
            #Write_Flash_Photo_fast(3926,'PH1')#160*80彩色图片，占用100个Page
            #Write_Flash_Photo_fast(4026,'N24X33P')#24*33分辨率数码管图像，占用12个Page
            #Write_Flash_Photo_fast(4038,'MP1')#160*80单色图片，占用7个Page
        else:
            Device_State=0#可以正常连接
            
def MSN_Device_1_State_machine():#MSN设备1的循环状态机
    global State_machine,key_eff,key_on,State_change,s1,s2,s3,color_use,Label2, photo_path1,photo_path2,write_path1,write_path2,LCD_Change_use,LCD_Change_now,write_path3,write_path4,Img_data_use
    #print("State_machine"+str(State_machine))
    #if write_path1==1:
    if LCD_Change_now!=LCD_Change_use:#显示方向与设置不符合
        LCD_Change_now=LCD_Change_use
        LCD_State(LCD_Change_now)#配置显示方向
        State_change=1
        
    color_La='#{:02x}{:02x}{:02x}'.format(int(s1.get())*8,int(s2.get())*4,int(s3.get())*8)
    Label2.config(bg=color_La)
    
    
    if(write_path1==1):
        LCD_State(LCD_Change_now)#配置显示方向
        Write_Flash_hex_fast(31616,Img_data_use)
        write_path1=0
        State_change=1
    if(write_path2==1):
        LCD_State(LCD_Change_now)#配置显示方向
        Write_Flash_Photo_fast(0,photo_path2)#固件烧录
        write_path2=0
        State_change=1
        
    if(write_path3==1):
        LCD_State(LCD_Change_now)#配置显示方向
        Write_Flash_hex_fast(31616+430,Img_data_use)
        write_path3=0
        State_change=1 
    if(write_path4==1):
        LCD_State(LCD_Change_now)#配置显示方向
        Write_Flash_hex_fast(4096,Img_data_use)#动图烧录
        #Write_Flash_hex_fast_Max(31616+215,Img_data_use)
        write_path4=0
        State_change=1
        
        
    
    if(State_machine==0):
        show_gif()
    elif(State_machine==1):
        show_PC_state(BLUE,BLACK)
    elif(State_machine==2):
        color_now=int(s1.get())*2048+int(s2.get())*32+int(s3.get())
        if color_now!=color_use:
            color_use=color_now
            #State_change=1
            RGB_LED_Set_All_Colour(int(s1.get()*2),int(s2.get()),int(s3.get()*1.6))#
        show_PC_state(color_use,BLACK)
    elif(State_machine==3):
        show_Photo1()
    elif(State_machine==4):
        show_PC_time()
    elif(State_machine==5):
        show_PC_Screen()
        


print("该设备具有"+str(psutil.cpu_count(logical=False))+"个内核和"+str(psutil.cpu_count())+"个逻辑处理器")
print("该CPU主频为"+str(round((psutil.cpu_freq().current/1000),1))+"GHZ")
print("当前CPU占用率为"+str(psutil.cpu_percent())+"%")#并不准确
mem = psutil.virtual_memory()
print("该设备具有"+str(round(mem.total/(1024*1024*1024)))+"GB的内存")
print("当前内存占用率为"+str(mem.percent)+"%")
print("开始运行时间"+datetime.fromtimestamp(psutil.boot_time()).strftime("%Y-%m-%d %H:%M:%S"))
battery = psutil.sensors_battery()
# 初始化摄像头（默认主显示器）
camera = dxcam.create(device_idx=0)
if battery!=None :
    print("电池剩余电量"+str(battery.percent)+"%")
#if battery.power_plugged:
#	print("已连接电源线")
#else:
#	print("已断开电源线")

D=0



CPU=0
FC=BLUE
BC=BLACK
key_on=0
key_eff=0
State_change=1#状态发生变化
gif_num=0
gif_data_num=0
gif_blk_num=0

State_machine=5#定义初始状态
Device_State=0#初始为未连接
LCD_Change_use=0#初始显示方向
LCD_Change_now=0
color_use=RED
write_path1=0
write_path2=0
write_path3=0
write_path4=0
photo_path1=""
photo_path2=""
photo_path3=""
photo_path4=""

Thread2=threading.Thread(target=UI_Page)

try:#尝试建立用户界面
    Thread2.start()
except:
    print("警告,无法创建用户界面")
    
while(1):
    D=D+1
    #print(D)
    #print(Thread2.is_alive())
    if Thread2.is_alive()==False:#检测到用户界面被关闭
        try:#尝试关闭屏幕截屏线程
            Thread1.stop()
        except:
            print("警告,无法关闭截图线程")
        try:#尝试关闭屏幕截屏线程
            Thread2.stop()
        except:
            print("警告,无法关闭截图线程")
        sys.exit() #退出程序
        break
    else:
        if(Device_State==0):#未检测到设备
            Get_MSN_Device()#尝试获取MSN设备
        #print("Waiting")
        elif(Device_State==1):#已检测到设备
            MSN_Device_1_State_machine()
            
            #time.sleep(10)
        #print("OK")
    
    

    
  
            
 
