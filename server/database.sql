USE master
IF EXISTS(select * from sys.databases where name='Conduit')
DROP DATABASE Conduit
GO
CREATE DATABASE Conduit
GO
USE [Conduit]
GO
/****** Object:  Table [dbo].[Followings]    Script Date: 6/8/2017 5:52:08 PM ******/
SET ANSI_NULLS ON
GO
SET QUOTED_IDENTIFIER ON
GO
CREATE TABLE [dbo].[Followings](
	[FollowingId] [int] NOT NULL,
	[FollowerId] [int] NOT NULL
) ON [PRIMARY]
GO
/****** Object:  Table [dbo].[Users]    Script Date: 6/8/2017 5:52:08 PM ******/
SET ANSI_NULLS ON
GO
SET QUOTED_IDENTIFIER ON
GO
CREATE TABLE [dbo].[Users](
	[Id] [int] IDENTITY(1,1) NOT NULL,
	[Email] [nvarchar](50) NOT NULL,
	[Token] [varchar](250) NOT NULL,
	[UserName] [nvarchar](150) NOT NULL,
	[Bio] [nvarchar](max) NULL,
	[Image] [nvarchar](250) NULL,
 CONSTRAINT [PK_Users] PRIMARY KEY CLUSTERED 
(
	[Id] ASC
)WITH (PAD_INDEX = OFF, STATISTICS_NORECOMPUTE = OFF, IGNORE_DUP_KEY = OFF, ALLOW_ROW_LOCKS = ON, ALLOW_PAGE_LOCKS = ON) ON [PRIMARY]
) ON [PRIMARY] TEXTIMAGE_ON [PRIMARY]
GO
/****** Object:  Index [IX_Followings]    Script Date: 6/8/2017 5:52:08 PM ******/
CREATE UNIQUE NONCLUSTERED INDEX [IX_Followings] ON [dbo].[Followings]
(
	[FollowingId] ASC,
	[FollowerId] ASC
)WITH (PAD_INDEX = OFF, STATISTICS_NORECOMPUTE = OFF, SORT_IN_TEMPDB = OFF, IGNORE_DUP_KEY = OFF, DROP_EXISTING = OFF, ONLINE = OFF, ALLOW_ROW_LOCKS = ON, ALLOW_PAGE_LOCKS = ON) ON [PRIMARY]
GO
ALTER TABLE [dbo].[Followings]  WITH CHECK ADD  CONSTRAINT [FK_Followings_Users] FOREIGN KEY([FollowerId])
REFERENCES [dbo].[Users] ([Id])
GO
ALTER TABLE [dbo].[Followings] CHECK CONSTRAINT [FK_Followings_Users]
GO
ALTER TABLE [dbo].[Followings]  WITH CHECK ADD  CONSTRAINT [FK_Followings_Users1] FOREIGN KEY([FollowingId])
REFERENCES [dbo].[Users] ([Id])
GO
ALTER TABLE [dbo].[Followings] CHECK CONSTRAINT [FK_Followings_Users1]
GO

CREATE TABLE [dbo].[Articles](
	[Id] [int] IDENTITY(1,1) NOT NULL,
	[Slug] [nvarchar](250) NULL,
	[Title] [nvarchar](250) NOT NULL,
	[Description] [nvarchar](250) NOT NULL,
	[Body] [nvarchar](max) NOT NULL,
	[Created] [datetime] NOT NULL,
	[Updated] [datetime] NULL,
	[Author] [int] NOT NULL,
 CONSTRAINT [PK_Articles] PRIMARY KEY CLUSTERED 
(
	[Id] ASC
)WITH (PAD_INDEX = OFF, STATISTICS_NORECOMPUTE = OFF, IGNORE_DUP_KEY = OFF, ALLOW_ROW_LOCKS = ON, ALLOW_PAGE_LOCKS = ON) ON [PRIMARY]
) ON [PRIMARY] TEXTIMAGE_ON [PRIMARY]
GO

ALTER TABLE [dbo].[Articles]  WITH CHECK ADD  CONSTRAINT [FK_Articles_Users] FOREIGN KEY([Author])
REFERENCES [dbo].[Users] ([Id])
GO

ALTER TABLE [dbo].[Articles] CHECK CONSTRAINT [FK_Articles_Users]
GO

SELECT 1