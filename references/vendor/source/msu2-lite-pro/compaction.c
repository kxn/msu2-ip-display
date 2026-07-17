//gcc编译为动态链接库指令：gcc -shared -o compaction.dll compaction.c
#include "stdio.h"
unsigned int MSN_compaction (unsigned short* RGB565,unsigned char* Out_Data,unsigned int num)//回复数据长度u32 压缩函数（u16 原始数据，u8 空数组，数据长度）
{
	unsigned int j,k,m,len;
	unsigned short a_data,b_data;
	unsigned char i,a,b,s;
	k=num>>7;
	len=0;
	for(j=0;j<k;j++)
	{
		i=0;
		while(i<128)
		{
			a=0;
			a_data=*RGB565;
			for(s=0;s<15;s++)
			{
				if(i<128)
				{
					if((a_data==*RGB565))
					{
						a++;
						i++;
						RGB565++;
					}
					else break;
				}
			}
			b=0;
			if(i<128)
			{
				b_data=*RGB565;
				for(s=0;s<15;s++)
				{
					if(i<128)
					{
						if(b_data==*RGB565)
						{
							b++;
							i++;
							RGB565++;
						}
						else break;
					}
					
				}
			}
			*Out_Data=9;Out_Data++;
			*Out_Data=a*16+b;Out_Data++;
			*Out_Data=a_data >>8;Out_Data++;
			*Out_Data=a_data&0xFF;Out_Data++;
			*Out_Data=b_data >>8;Out_Data++;
			*Out_Data=b_data&0xFF;Out_Data++;
			len+=6;
		}
		//if(j<k-1) RGB565=RGB565+128;//将指针偏移到下一目标区域
	}
	return len;
}
